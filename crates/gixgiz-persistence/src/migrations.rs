use std::{path::PathBuf, time::Duration};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, backup::Backup, params};
use uuid::Uuid;

use crate::{DataRoot, PersistenceError};

/// Latest repository-owned schema version supported by this binary.
pub const CURRENT_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Copy)]
struct Migration {
    version: u32,
    name: &'static str,
    sql: &'static str,
    irreversible: bool,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial",
        sql: include_str!("../migrations/0001_initial.sql"),
        irreversible: false,
    },
    Migration {
        version: 2,
        name: "runtime_policy",
        sql: include_str!("../migrations/0002_runtime_policy.sql"),
        irreversible: false,
    },
];

pub(crate) fn run(connection: &mut Connection, root: &DataRoot) -> Result<(), PersistenceError> {
    run_with(connection, root, MIGRATIONS)
}

fn run_with(
    connection: &mut Connection,
    root: &DataRoot,
    migrations: &[Migration],
) -> Result<(), PersistenceError> {
    validate_migration_set(migrations)?;
    let supported = migrations.last().map_or(0, |migration| migration.version);
    let mut current = schema_version(connection)?;
    if current > supported {
        return Err(PersistenceError::SchemaTooNew {
            found: current,
            supported,
        });
    }
    validate_applied_migrations(connection, migrations, current)?;

    for migration in migrations {
        if migration.version <= current {
            continue;
        }
        if migration.irreversible && current > 0 {
            create_verified_backup(connection, root, current, migration.version)?;
        }
        apply_migration(connection, *migration)?;
        current = migration.version;
    }
    Ok(())
}

pub(crate) fn schema_version(connection: &Connection) -> Result<u32, PersistenceError> {
    connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|source| PersistenceError::sqlite("read_schema_version", source))
}

fn validate_migration_set(migrations: &[Migration]) -> Result<(), PersistenceError> {
    let ordered = migrations
        .iter()
        .enumerate()
        .all(|(index, migration)| migration.version as usize == index + 1);
    if ordered {
        Ok(())
    } else {
        Err(PersistenceError::MigrationLedger)
    }
}

fn validate_applied_migrations(
    connection: &Connection,
    migrations: &[Migration],
    current: u32,
) -> Result<(), PersistenceError> {
    if current == 0 {
        return Ok(());
    }

    let ledger_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations')",
            [],
            |row| row.get(0),
        )
        .map_err(|source| PersistenceError::sqlite("validate_migration_ledger", source))?;
    if !ledger_exists {
        return Err(PersistenceError::MigrationLedger);
    }

    let (ledger_max, ledger_count): (u32, u32) = connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0), COUNT(*) FROM schema_migrations",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|source| PersistenceError::sqlite("validate_migration_ledger", source))?;
    if ledger_max != current || ledger_count != current {
        return Err(PersistenceError::MigrationLedger);
    }

    for migration in migrations
        .iter()
        .filter(|migration| migration.version <= current)
    {
        let applied_name = connection
            .query_row(
                "SELECT name FROM schema_migrations WHERE version = ?1",
                params![migration.version],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| PersistenceError::sqlite("validate_migration_ledger", source))?;
        if applied_name.as_deref() != Some(migration.name) {
            return Err(PersistenceError::MigrationLedger);
        }
    }
    Ok(())
}

fn apply_migration(
    connection: &mut Connection,
    migration: Migration,
) -> Result<(), PersistenceError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|source| PersistenceError::migration(migration.version, source))?;
    let applied_at_unix_ms = unix_timestamp_millis();
    transaction
        .execute_batch(migration.sql)
        .map_err(|source| PersistenceError::migration(migration.version, source))?;
    transaction
        .execute(
            "INSERT INTO schema_migrations (version, name, applied_at_unix_ms) VALUES (?1, ?2, ?3)",
            params![migration.version, migration.name, applied_at_unix_ms],
        )
        .map_err(|source| PersistenceError::migration(migration.version, source))?;
    transaction
        .execute(
            "INSERT INTO application_metadata (key, value, updated_at_unix_ms)
             VALUES ('last_migrated_application_version', ?1, ?2)
             ON CONFLICT(key) DO UPDATE SET
                 value = excluded.value,
                 updated_at_unix_ms = excluded.updated_at_unix_ms",
            params![env!("CARGO_PKG_VERSION"), applied_at_unix_ms],
        )
        .map_err(|source| PersistenceError::migration(migration.version, source))?;
    transaction
        .pragma_update(None, "user_version", migration.version)
        .map_err(|source| PersistenceError::migration(migration.version, source))?;
    transaction
        .commit()
        .map_err(|source| PersistenceError::migration(migration.version, source))
}

fn create_verified_backup(
    source: &Connection,
    root: &DataRoot,
    from_version: u32,
    to_version: u32,
) -> Result<PathBuf, PersistenceError> {
    let path = root.backups_dir().join(format!(
        "gixgiz-v{from_version}-before-v{to_version}-{}.db",
        Uuid::new_v4()
    ));
    let mut destination = Connection::open(&path).map_err(PersistenceError::backup)?;
    {
        let backup = Backup::new(source, &mut destination).map_err(PersistenceError::backup)?;
        backup
            .run_to_completion(64, Duration::from_millis(5), None)
            .map_err(PersistenceError::backup)?;
    }

    let integrity: String = destination
        .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
        .map_err(PersistenceError::backup)?;
    let version = schema_version(&destination).map_err(|error| match error {
        PersistenceError::Database { source, .. }
        | PersistenceError::Locked { source }
        | PersistenceError::Corrupt { source } => PersistenceError::backup(source),
        _ => PersistenceError::backup(rusqlite::Error::InvalidQuery),
    })?;
    if integrity != "ok" || version != from_version {
        return Err(PersistenceError::backup(rusqlite::Error::InvalidQuery));
    }
    Ok(path)
}

fn unix_timestamp_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND_MIGRATION: Migration = Migration {
        version: 2,
        name: "second",
        sql: "CREATE TABLE second_table (id INTEGER PRIMARY KEY);",
        irreversible: false,
    };

    const THIRD_IRREVERSIBLE_MIGRATION: Migration = Migration {
        version: 3,
        name: "third_irreversible",
        sql: "CREATE TABLE third_table (id INTEGER PRIMARY KEY);",
        irreversible: true,
    };

    fn open_test_database() -> (tempfile::TempDir, DataRoot, Connection) {
        let temporary = tempfile::tempdir().expect("temporary directory is available");
        let root =
            DataRoot::from_override(temporary.path().join("root")).expect("test root initializes");
        let mut connection = Connection::open(root.database_path()).expect("database opens");
        run_with(&mut connection, &root, &[MIGRATIONS[0]]).expect("initial migration runs");
        (temporary, root, connection)
    }

    #[test]
    fn failed_migration_rolls_back_schema_and_version() {
        let (_temporary, root, mut connection) = open_test_database();
        let failing = Migration {
            version: 2,
            name: "failing",
            sql: "CREATE TABLE partial_table (id INTEGER); INSERT INTO missing_table VALUES (1);",
            irreversible: false,
        };

        let error = run_with(&mut connection, &root, &[MIGRATIONS[0], failing])
            .expect_err("migration must fail");

        assert!(matches!(
            error,
            PersistenceError::Migration { version: 2, .. }
        ));
        assert_eq!(schema_version(&connection).expect("version reads"), 1);
        let version_two_entries: u32 = connection
            .query_row(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = 2",
                [],
                |row| row.get(0),
            )
            .expect("ledger query succeeds");
        assert_eq!(version_two_entries, 0);
        let partial_exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'partial_table')",
                [],
                |row| row.get(0),
            )
            .expect("schema query succeeds");
        assert!(!partial_exists);
    }

    #[test]
    fn migrations_run_in_order_and_record_the_ledger() {
        let (_temporary, root, mut connection) = open_test_database();

        run_with(&mut connection, &root, &[MIGRATIONS[0], SECOND_MIGRATION])
            .expect("second migration runs");

        let versions: Vec<u32> = connection
            .prepare("SELECT version FROM schema_migrations ORDER BY version")
            .expect("ledger statement prepares")
            .query_map([], |row| row.get(0))
            .expect("ledger query succeeds")
            .collect::<Result<_, _>>()
            .expect("ledger rows decode");
        assert_eq!(versions, vec![1, 2]);
        assert_eq!(schema_version(&connection).expect("version reads"), 2);
    }

    #[test]
    fn irreversible_migration_creates_verified_prior_version_backup() {
        let (_temporary, root, mut connection) = open_test_database();
        let irreversible = Migration {
            irreversible: true,
            ..SECOND_MIGRATION
        };

        run_with(&mut connection, &root, &[MIGRATIONS[0], irreversible])
            .expect("irreversible migration runs after backup");

        let backups: Vec<PathBuf> = std::fs::read_dir(root.backups_dir())
            .expect("backup directory reads")
            .map(|entry| entry.expect("backup entry reads").path())
            .collect();
        assert_eq!(backups.len(), 1);
        let backup = Connection::open(&backups[0]).expect("backup opens");
        assert_eq!(schema_version(&backup).expect("backup version reads"), 1);
    }

    #[test]
    fn sequential_upgrade_backs_up_the_immediately_previous_schema() {
        let (_temporary, root, mut connection) = open_test_database();

        run_with(
            &mut connection,
            &root,
            &[
                MIGRATIONS[0],
                SECOND_MIGRATION,
                THIRD_IRREVERSIBLE_MIGRATION,
            ],
        )
        .expect("sequential migrations run");

        let backup_path = std::fs::read_dir(root.backups_dir())
            .expect("backup directory reads")
            .next()
            .expect("one backup exists")
            .expect("backup entry reads")
            .path();
        let backup = Connection::open(backup_path).expect("backup opens");
        assert_eq!(schema_version(&backup).expect("backup version reads"), 2);
        assert_eq!(schema_version(&connection).expect("live version reads"), 3);
    }
}
