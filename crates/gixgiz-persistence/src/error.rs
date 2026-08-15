use std::io;

use gixgiz_contracts::{
    CorrelationId, ErrorCategory, RecoveryAction, RecoveryGuidance, RequestId, SafeErrorPayload,
    ServiceHealthStatus,
};
use rusqlite::ErrorCode;
use thiserror::Error;

/// Typed persistence failures with raw causes retained only inside Rust.
#[derive(Debug, Error)]
pub enum PersistenceError {
    /// The default per-user data root could not be resolved.
    #[error("the per-user data root is unavailable")]
    DataRootUnavailable,
    /// A filesystem operation failed beneath the configured data root.
    #[error("the persistence data root could not be prepared")]
    PathIo {
        /// Original I/O cause, retained for sanitized diagnostics only.
        #[source]
        source: io::Error,
    },
    /// A canonical child path resolved outside the configured root.
    #[error("a persistence path escaped the configured data root")]
    PathEscapesRoot,
    /// SQLite refused access because the database is read-only or denied.
    #[error("the persistence database is not writable")]
    PermissionDenied {
        /// Original SQLite cause, retained for sanitized diagnostics only.
        #[source]
        source: rusqlite::Error,
    },
    /// Another platform-core process already owns this database.
    #[error("the persistence database is already owned")]
    AlreadyOwned {
        /// Original lock-file cause, retained for sanitized diagnostics only.
        #[source]
        source: io::Error,
    },
    /// SQLite reported a lock or bounded busy-timeout failure.
    #[error("the persistence database is temporarily locked")]
    Locked {
        /// Original SQLite cause, retained for sanitized diagnostics only.
        #[source]
        source: rusqlite::Error,
    },
    /// SQLite reported corrupt or non-database content.
    #[error("the persistence database failed integrity validation")]
    Corrupt {
        /// Original SQLite cause, retained for sanitized diagnostics only.
        #[source]
        source: rusqlite::Error,
    },
    /// SQLite could not open or operate on the database.
    #[error("the persistence database operation failed")]
    Database {
        /// Stable internal operation name.
        operation: &'static str,
        /// Original SQLite cause, retained for sanitized diagnostics only.
        #[source]
        source: rusqlite::Error,
    },
    /// The database schema is newer than this binary supports.
    #[error("the persistence schema is newer than this application supports")]
    SchemaTooNew {
        /// Version found in the database.
        found: u32,
        /// Highest version supported by this binary.
        supported: u32,
    },
    /// The migration ledger does not match the immutable migration set.
    #[error("the persistence migration ledger failed validation")]
    MigrationLedger,
    /// A transactional schema migration failed.
    #[error("persistence migration {version} failed")]
    Migration {
        /// Migration version that failed.
        version: u32,
        /// Original SQLite cause, retained for sanitized diagnostics only.
        #[source]
        source: rusqlite::Error,
    },
    /// A required pre-migration backup could not be created or verified.
    #[error("the required persistence backup failed")]
    Backup {
        /// Original SQLite cause, retained for sanitized diagnostics only.
        #[source]
        source: rusqlite::Error,
    },
    /// A typed repository record failed bounded validation.
    #[error("a persistence record is invalid")]
    InvalidRecord {
        /// Stable field name that failed validation.
        field: &'static str,
    },
    /// A requested typed repository record does not exist.
    #[error("a persistence record was not found")]
    RecordNotFound {
        /// Stable repository-owned entity name.
        entity: &'static str,
    },
    /// A repository write lost an optimistic state or revision race.
    #[error("a persistence record changed before the requested write")]
    RecordConflict {
        /// Stable repository-owned entity name.
        entity: &'static str,
    },
    /// The in-process single connection lock was poisoned.
    #[error("the persistence connection is unavailable")]
    ConnectionUnavailable,
}

impl PersistenceError {
    pub(crate) fn path_io(source: io::Error) -> Self {
        Self::PathIo { source }
    }

    pub(crate) fn sqlite(operation: &'static str, source: rusqlite::Error) -> Self {
        match sqlite_code(&source) {
            Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => Self::Locked { source },
            Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) => Self::Corrupt { source },
            Some(ErrorCode::PermissionDenied | ErrorCode::ReadOnly) => {
                Self::PermissionDenied { source }
            }
            _ => Self::Database { operation, source },
        }
    }

    pub(crate) fn migration(version: u32, source: rusqlite::Error) -> Self {
        Self::Migration { version, source }
    }

    pub(crate) fn backup(source: rusqlite::Error) -> Self {
        Self::Backup { source }
    }

    /// Returns the conservative service-health state for this failure.
    #[must_use]
    pub const fn health_status(&self) -> ServiceHealthStatus {
        match self {
            Self::DataRootUnavailable
            | Self::PathIo { .. }
            | Self::PermissionDenied { .. }
            | Self::AlreadyOwned { .. }
            | Self::Locked { .. }
            | Self::ConnectionUnavailable => ServiceHealthStatus::Unavailable,
            Self::PathEscapesRoot
            | Self::Corrupt { .. }
            | Self::SchemaTooNew { .. }
            | Self::MigrationLedger
            | Self::Migration { .. }
            | Self::Backup { .. }
            | Self::Database { .. }
            | Self::InvalidRecord { .. }
            | Self::RecordNotFound { .. }
            | Self::RecordConflict { .. } => ServiceHealthStatus::Failed,
        }
    }

    /// Returns a safe health explanation without paths, SQL, or raw causes.
    #[must_use]
    pub fn safe_health_message(&self) -> &'static str {
        match self {
            Self::AlreadyOwned { .. } | Self::Locked { .. } => {
                "Persistence is currently owned or locked by another operation."
            }
            Self::Corrupt { .. } => "Persistence failed an integrity check.",
            Self::SchemaTooNew { .. } => {
                "Persistence was created by a newer incompatible GixGiz version."
            }
            Self::Migration { .. } | Self::MigrationLedger | Self::Backup { .. } => {
                "Persistence could not complete a required schema migration safely."
            }
            Self::DataRootUnavailable
            | Self::PathIo { .. }
            | Self::PermissionDenied { .. }
            | Self::ConnectionUnavailable => {
                "Persistence is unavailable in the current per-user data location."
            }
            Self::PathEscapesRoot
            | Self::Database { .. }
            | Self::InvalidRecord { .. }
            | Self::RecordNotFound { .. }
            | Self::RecordConflict { .. } => "Persistence failed to initialize safely.",
        }
    }

    /// Maps an internal persistence failure to a stable boundary-safe payload.
    #[must_use]
    pub fn to_safe_payload(
        &self,
        correlation_id: CorrelationId,
        request_id: RequestId,
    ) -> SafeErrorPayload {
        let (category, code, message, action, recovery) = match self {
            Self::PathIo { source } if source.kind() == io::ErrorKind::PermissionDenied => (
                ErrorCategory::PermissionDenied,
                "persistence.permission_denied",
                "GixGiz cannot access its per-user data directory.",
                RecoveryAction::CheckPrerequisites,
                "Check the data-directory permissions and retry.",
            ),
            Self::PermissionDenied { .. } => (
                ErrorCategory::PermissionDenied,
                "persistence.permission_denied",
                "GixGiz cannot write to its local database.",
                RecoveryAction::CheckPrerequisites,
                "Check the data-directory permissions and retry.",
            ),
            Self::AlreadyOwned { .. } | Self::Locked { .. } => (
                ErrorCategory::Unavailable,
                "persistence.locked",
                "GixGiz persistence is temporarily unavailable.",
                RecoveryAction::Retry,
                "Close other GixGiz core processes, then retry.",
            ),
            Self::Corrupt { .. } => (
                ErrorCategory::IntegrityFailure,
                "persistence.corrupt",
                "GixGiz persistence failed an integrity check.",
                RecoveryAction::ContactSupport,
                "Keep the database and backups unchanged and collect sanitized diagnostics.",
            ),
            Self::SchemaTooNew { .. } => (
                ErrorCategory::IncompatibleVersion,
                "persistence.schema_too_new",
                "The local data was created by a newer GixGiz version.",
                RecoveryAction::CheckPrerequisites,
                "Use the same or a newer GixGiz version. Automatic downgrade is disabled.",
            ),
            Self::Migration { .. } | Self::MigrationLedger | Self::Backup { .. } => (
                ErrorCategory::IntegrityFailure,
                "persistence.migration_failed",
                "GixGiz could not update local data safely.",
                RecoveryAction::ContactSupport,
                "Keep the database and backups unchanged and collect sanitized diagnostics.",
            ),
            Self::InvalidRecord { .. } => (
                ErrorCategory::InvalidInput,
                "persistence.invalid_record",
                "The persistence record is invalid.",
                RecoveryAction::NoAction,
                "Correct the record and retry.",
            ),
            Self::RecordNotFound { .. } => (
                ErrorCategory::InvalidInput,
                "persistence.record_not_found",
                "The requested local record was not found.",
                RecoveryAction::NoAction,
                "Refresh the current state before retrying.",
            ),
            Self::RecordConflict { .. } => (
                ErrorCategory::Conflict,
                "persistence.record_conflict",
                "The local record changed before the request completed.",
                RecoveryAction::Retry,
                "Refresh the current state and retry the action.",
            ),
            Self::DataRootUnavailable
            | Self::PathIo { .. }
            | Self::PathEscapesRoot
            | Self::Database { .. }
            | Self::ConnectionUnavailable => (
                ErrorCategory::Unavailable,
                "persistence.unavailable",
                "GixGiz persistence is unavailable.",
                RecoveryAction::Retry,
                "Restart GixGiz and retry.",
            ),
        };

        SafeErrorPayload::new(
            category,
            code,
            message,
            RecoveryGuidance {
                action,
                message: recovery.to_owned(),
            },
            correlation_id,
            request_id,
        )
    }
}

fn sqlite_code(error: &rusqlite::Error) -> Option<ErrorCode> {
    match error {
        rusqlite::Error::SqliteFailure(details, _) => Some(details.code),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_errors_map_without_raw_paths() {
        let correlation_id = CorrelationId::new();
        let request_id = RequestId::new();
        let error = PersistenceError::path_io(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "D:\\private\\secret.db",
        ));
        let payload = error.to_safe_payload(correlation_id, request_id);

        assert_eq!(payload.category, ErrorCategory::PermissionDenied);
        assert_eq!(payload.code, "persistence.permission_denied");
        assert_eq!(payload.correlation_id, correlation_id);
        assert_eq!(payload.request_id, request_id);
        assert!(!payload.to_string().contains("private"));
        assert!(!payload.to_string().contains("secret.db"));
    }
}
