use std::{
    fs::{self, File, OpenOptions},
    io,
    sync::{Arc, Mutex, TryLockError},
    time::Duration,
};

use rusqlite::{Connection, Transaction, TransactionBehavior};

use crate::{
    AuditEventRepository, DataRoot, JobMetadataRepository, PersistenceError,
    PlatformMetadataRepository, RuntimePolicyRepository, SettingsRepository, migrations,
};

const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_BUSY_TIMEOUT: Duration = Duration::from_secs(30);

/// SQLite open policy used by production and bounded contention tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistenceOptions {
    busy_timeout: Duration,
}

impl PersistenceOptions {
    /// Creates options with a SQLite busy timeout capped at 30 seconds.
    #[must_use]
    pub fn new(busy_timeout: Duration) -> Self {
        Self {
            busy_timeout: busy_timeout.min(MAX_BUSY_TIMEOUT),
        }
    }

    /// Returns the configured busy timeout.
    #[must_use]
    pub const fn busy_timeout(self) -> Duration {
        self.busy_timeout
    }
}

impl Default for PersistenceOptions {
    fn default() -> Self {
        Self::new(DEFAULT_BUSY_TIMEOUT)
    }
}

/// Observed SQLite safety configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatabaseConfiguration {
    /// Whether SQLite foreign-key enforcement is enabled.
    pub foreign_keys: bool,
    /// Active SQLite journal mode, normalized to lowercase.
    pub journal_mode: String,
    /// Configured busy timeout in milliseconds.
    pub busy_timeout_millis: u64,
}

/// Successful persistence health evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistenceHealth {
    /// Current schema version verified by the health check.
    pub schema_version: u32,
}

/// Single-owner SQLite service and typed repository factory.
#[derive(Clone)]
pub struct Persistence {
    inner: Arc<DatabaseInner>,
}

struct DatabaseInner {
    connection: Mutex<Connection>,
    busy_timeout: Duration,
    root: DataRoot,
    _owner_lock: OwnerLock,
}

impl Persistence {
    /// Opens the database, applies safety configuration, and runs migrations.
    pub fn open(root: DataRoot) -> Result<Self, PersistenceError> {
        Self::open_with_options(root, PersistenceOptions::default())
    }

    /// Opens with an explicit busy timeout, primarily for bounded tests.
    pub fn open_with_options(
        root: DataRoot,
        options: PersistenceOptions,
    ) -> Result<Self, PersistenceError> {
        let owner_lock = OwnerLock::acquire(root.owner_lock_path())?;
        let mut connection = Connection::open(root.database_path())
            .map_err(|source| PersistenceError::sqlite("open", source))?;
        configure(&connection, options)?;
        migrations::run(&mut connection, &root)?;

        let persistence = Self {
            inner: Arc::new(DatabaseInner {
                connection: Mutex::new(connection),
                busy_timeout: options.busy_timeout(),
                root,
                _owner_lock: owner_lock,
            }),
        };
        persistence.health_check()?;
        Ok(persistence)
    }

    /// Returns the canonical data root without exposing a database handle.
    #[must_use]
    pub fn data_root(&self) -> &DataRoot {
        &self.inner.root
    }

    /// Verifies that the schema is readable and writable without retaining data.
    pub fn health_check(&self) -> Result<PersistenceHealth, PersistenceError> {
        let mut connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| PersistenceError::ConnectionUnavailable)?;
        connection
            .query_row("SELECT 1 FROM schema_migrations LIMIT 1", [], |row| {
                row.get::<_, u8>(0)
            })
            .map_err(|source| PersistenceError::sqlite("health_read", source))?;
        let schema_version = migrations::schema_version(&connection)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| PersistenceError::sqlite("health_write", source))?;
        transaction
            .execute(
                "UPDATE application_metadata SET value = value
                 WHERE key = 'last_migrated_application_version'",
                [],
            )
            .map_err(|source| PersistenceError::sqlite("health_write", source))?;
        transaction
            .rollback()
            .map_err(|source| PersistenceError::sqlite("health_rollback", source))?;
        Ok(PersistenceHealth { schema_version })
    }

    /// Returns the verified SQLite configuration.
    pub fn configuration(&self) -> Result<DatabaseConfiguration, PersistenceError> {
        self.with_connection(|connection| {
            let foreign_keys: bool = connection
                .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
                .map_err(|source| PersistenceError::sqlite("read_configuration", source))?;
            let journal_mode: String = connection
                .query_row("PRAGMA journal_mode", [], |row| row.get(0))
                .map_err(|source| PersistenceError::sqlite("read_configuration", source))?;
            let busy_timeout_millis: i64 = connection
                .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
                .map_err(|source| PersistenceError::sqlite("read_configuration", source))?;
            let busy_timeout_millis =
                u64::try_from(busy_timeout_millis).map_err(|_| PersistenceError::Database {
                    operation: "read_configuration",
                    source: rusqlite::Error::IntegralValueOutOfRange(0, busy_timeout_millis),
                })?;
            Ok(DatabaseConfiguration {
                foreign_keys,
                journal_mode: journal_mode.to_ascii_lowercase(),
                busy_timeout_millis,
            })
        })
    }

    /// Returns the bounded application-metadata repository.
    #[must_use]
    pub fn platform_metadata(&self) -> PlatformMetadataRepository {
        PlatformMetadataRepository::new(self.clone())
    }

    /// Returns the bounded non-secret settings repository.
    #[must_use]
    pub fn settings(&self) -> SettingsRepository {
        SettingsRepository::new(self.clone())
    }

    /// Returns the provider-neutral runtime ownership and consent repository.
    #[must_use]
    pub fn runtime_policy(&self) -> RuntimePolicyRepository {
        RuntimePolicyRepository::new(self.clone())
    }

    /// Returns the minimal durable-job metadata repository.
    #[must_use]
    pub fn jobs(&self) -> JobMetadataRepository {
        JobMetadataRepository::new(self.clone())
    }

    /// Returns the append-only, redaction-safe audit repository.
    #[must_use]
    pub fn audit_events(&self) -> AuditEventRepository {
        AuditEventRepository::new(self.clone())
    }

    pub(crate) fn with_connection<T>(
        &self,
        operation: impl FnOnce(&Connection) -> Result<T, PersistenceError>,
    ) -> Result<T, PersistenceError> {
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| PersistenceError::ConnectionUnavailable)?;
        operation(&connection)
    }

    pub(crate) fn try_with_connection<T>(
        &self,
        operation: impl FnOnce(&Connection) -> Result<T, PersistenceError>,
    ) -> Result<T, PersistenceError> {
        let connection = self
            .inner
            .connection
            .try_lock()
            .map_err(|error| match error {
                TryLockError::Poisoned(_) | TryLockError::WouldBlock => {
                    PersistenceError::ConnectionUnavailable
                }
            })?;
        operation(&connection)
    }

    pub(crate) fn with_write_transaction<T>(
        &self,
        operation: impl FnOnce(&Transaction<'_>) -> Result<T, PersistenceError>,
    ) -> Result<T, PersistenceError> {
        let mut connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| PersistenceError::ConnectionUnavailable)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| PersistenceError::sqlite("begin_write", source))?;
        let result = operation(&transaction)?;
        transaction
            .commit()
            .map_err(|source| PersistenceError::sqlite("commit_write", source))?;
        Ok(result)
    }

    pub(crate) fn try_with_write_transaction<T>(
        &self,
        operation: impl FnOnce(&Connection) -> Result<T, PersistenceError>,
    ) -> Result<T, PersistenceError> {
        let connection = self
            .inner
            .connection
            .try_lock()
            .map_err(|error| match error {
                TryLockError::Poisoned(_) | TryLockError::WouldBlock => {
                    PersistenceError::ConnectionUnavailable
                }
            })?;
        connection
            .busy_timeout(Duration::ZERO)
            .map_err(|source| PersistenceError::sqlite("configure_immediate_write", source))?;
        let begin_result = connection.execute_batch("BEGIN IMMEDIATE");
        let restore_result = connection.busy_timeout(self.inner.busy_timeout);
        match (begin_result, restore_result) {
            (Err(source), _) => return Err(PersistenceError::sqlite("begin_write", source)),
            (Ok(()), Err(source)) => {
                let _ = connection.execute_batch("ROLLBACK");
                return Err(PersistenceError::sqlite("restore_busy_timeout", source));
            }
            (Ok(()), Ok(())) => {}
        }

        match operation(&connection) {
            Ok(result) => match connection.execute_batch("COMMIT") {
                Ok(()) => Ok(result),
                Err(source) => {
                    let _ = connection.execute_batch("ROLLBACK");
                    Err(PersistenceError::sqlite("commit_write", source))
                }
            },
            Err(error) => {
                let _ = connection.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }
}

fn configure(connection: &Connection, options: PersistenceOptions) -> Result<(), PersistenceError> {
    connection
        .busy_timeout(options.busy_timeout())
        .map_err(|source| PersistenceError::sqlite("configure_busy_timeout", source))?;
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(|source| PersistenceError::sqlite("configure_foreign_keys", source))?;
    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
        .map_err(|source| PersistenceError::sqlite("configure_wal", source))?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(PersistenceError::Database {
            operation: "configure_wal",
            source: rusqlite::Error::InvalidQuery,
        });
    }
    Ok(())
}

struct OwnerLock {
    file: Option<File>,
    path: std::path::PathBuf,
}

impl OwnerLock {
    fn acquire(path: std::path::PathBuf) -> Result<Self, PersistenceError> {
        match open_owner_lock(&path) {
            Ok(file) => Ok(Self {
                file: Some(file),
                path,
            }),
            Err(source) if is_owner_conflict(&source) => {
                Err(PersistenceError::AlreadyOwned { source })
            }
            Err(source) => Err(PersistenceError::path_io(source)),
        }
    }
}

impl Drop for OwnerLock {
    fn drop(&mut self) {
        let _ = self.file.take();
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(windows)]
fn open_owner_lock(path: &std::path::Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;

    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .share_mode(0)
        .open(path)
}

#[cfg(not(windows))]
fn open_owner_lock(path: &std::path::Path) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

#[cfg(windows)]
fn is_owner_conflict(error: &io::Error) -> bool {
    matches!(error.raw_os_error(), Some(32 | 33))
}

#[cfg(not(windows))]
fn is_owner_conflict(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::AlreadyExists
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_multi_statement_write_rolls_back() {
        let temporary = tempfile::tempdir().expect("temporary directory is available");
        let root =
            DataRoot::from_override(temporary.path().join("root")).expect("test root initializes");
        let persistence = Persistence::open(root).expect("database opens");

        let result = persistence.with_write_transaction(|transaction| {
            transaction
                .execute(
                    "INSERT INTO settings (key, value, updated_at_unix_ms) VALUES ('first', 'value', 1)",
                    [],
                )
                .map_err(|source| PersistenceError::sqlite("test_insert", source))?;
            transaction
                .execute(
                    "INSERT INTO settings (key, value, updated_at_unix_ms) VALUES ('first', 'duplicate', 2)",
                    [],
                )
                .map_err(|source| PersistenceError::sqlite("test_insert", source))?;
            Ok(())
        });

        assert!(result.is_err());
        assert_eq!(
            persistence
                .settings()
                .get("first")
                .expect("setting query succeeds"),
            None
        );
    }

    #[test]
    fn try_connection_paths_fail_fast_during_in_process_contention() {
        let temporary = tempfile::tempdir().expect("temporary directory is available");
        let root =
            DataRoot::from_override(temporary.path().join("root")).expect("test root initializes");
        let persistence = Persistence::open(root).expect("database opens");
        let _guard = persistence
            .inner
            .connection
            .lock()
            .expect("test owns connection lock");

        assert!(matches!(
            persistence.try_with_connection(|_| Ok(())),
            Err(PersistenceError::ConnectionUnavailable)
        ));
        assert!(matches!(
            persistence.try_with_write_transaction(|_| Ok(())),
            Err(PersistenceError::ConnectionUnavailable)
        ));
    }
}
