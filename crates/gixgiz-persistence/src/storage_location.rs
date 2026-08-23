//! Durable user-selected storage locations.
//!
//! Absence of a row means the built-in default applies. A stored row therefore
//! always represents a deliberate user choice, never an inferred one.

use rusqlite::{OptionalExtension, params};

use crate::{Persistence, PersistenceError};

const KIND_BYTES_MAX: usize = 32;
const CATEGORY_BYTES_MAX: usize = 32;
const PATH_BYTES_MAX: usize = 240;

/// One durable storage-location row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedStorageLocation {
    /// Which artifact the location applies to.
    pub kind: String,
    /// Who owns the destination.
    pub category: String,
    /// Absolute path for a user-selected location.
    pub path: Option<String>,
    /// When the user accepted reconfiguring an externally owned runtime.
    pub external_consent_at_unix_ms: Option<i64>,
    /// Last durable update.
    pub updated_at_unix_ms: i64,
}

/// Repository for durable storage-location settings.
#[derive(Clone)]
pub struct StorageLocationRepository {
    persistence: Persistence,
}

impl StorageLocationRepository {
    pub(crate) fn new(persistence: Persistence) -> Self {
        Self { persistence }
    }

    /// Creates or replaces one location in a single transaction.
    pub fn upsert(&self, location: &PersistedStorageLocation) -> Result<(), PersistenceError> {
        validate(location)?;
        self.persistence.try_with_write_transaction(|transaction| {
            transaction
                .execute(
                    "INSERT INTO storage_locations
                     (kind, category, path, external_consent_at_unix_ms, updated_at_unix_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(kind) DO UPDATE SET
                         category = excluded.category,
                         path = excluded.path,
                         external_consent_at_unix_ms =
                             excluded.external_consent_at_unix_ms,
                         updated_at_unix_ms = excluded.updated_at_unix_ms",
                    params![
                        location.kind,
                        location.category,
                        location.path,
                        location.external_consent_at_unix_ms,
                        location.updated_at_unix_ms,
                    ],
                )
                .map_err(|source| PersistenceError::sqlite("write_storage_location", source))?;
            Ok(())
        })
    }

    /// Removes one location so the built-in default applies again.
    pub fn clear(&self, kind: &str) -> Result<(), PersistenceError> {
        validate_text("storage_location_kind", kind, KIND_BYTES_MAX)?;
        self.persistence.try_with_write_transaction(|transaction| {
            transaction
                .execute(
                    "DELETE FROM storage_locations WHERE kind = ?1",
                    params![kind],
                )
                .map_err(|source| PersistenceError::sqlite("clear_storage_location", source))?;
            Ok(())
        })
    }

    /// Reads one location, or `None` when the default applies.
    pub fn get(&self, kind: &str) -> Result<Option<PersistedStorageLocation>, PersistenceError> {
        validate_text("storage_location_kind", kind, KIND_BYTES_MAX)?;
        self.persistence.try_with_connection(|connection| {
            connection
                .query_row(
                    "SELECT kind, category, path, external_consent_at_unix_ms, updated_at_unix_ms
                     FROM storage_locations WHERE kind = ?1",
                    params![kind],
                    decode,
                )
                .optional()
                .map_err(|source| PersistenceError::sqlite("read_storage_location", source))
        })
    }

    /// Lists every stored location in deterministic order.
    pub fn list(&self) -> Result<Vec<PersistedStorageLocation>, PersistenceError> {
        self.persistence.try_with_connection(|connection| {
            let mut statement = connection
                .prepare(
                    "SELECT kind, category, path, external_consent_at_unix_ms, updated_at_unix_ms
                     FROM storage_locations ORDER BY kind",
                )
                .map_err(|source| {
                    PersistenceError::sqlite("prepare_storage_location_list", source)
                })?;
            let rows = statement
                .query_map([], decode)
                .map_err(|source| PersistenceError::sqlite("read_storage_locations", source))?;
            let mut locations = Vec::new();
            for row in rows {
                locations.push(row.map_err(|source| {
                    PersistenceError::sqlite("decode_storage_location", source)
                })?);
            }
            Ok(locations)
        })
    }
}

fn decode(row: &rusqlite::Row<'_>) -> rusqlite::Result<PersistedStorageLocation> {
    Ok(PersistedStorageLocation {
        kind: row.get(0)?,
        category: row.get(1)?,
        path: row.get(2)?,
        external_consent_at_unix_ms: row.get(3)?,
        updated_at_unix_ms: row.get(4)?,
    })
}

fn validate(location: &PersistedStorageLocation) -> Result<(), PersistenceError> {
    validate_text("storage_location_kind", &location.kind, KIND_BYTES_MAX)?;
    validate_text(
        "storage_location_category",
        &location.category,
        CATEGORY_BYTES_MAX,
    )?;
    if let Some(path) = location.path.as_deref() {
        validate_text("storage_location_path", path, PATH_BYTES_MAX)?;
    }
    // The schema enforces this too; rejecting here keeps the error typed rather
    // than surfacing as a constraint violation.
    let user_selected = location.category == "user_selected";
    if user_selected != location.path.is_some() {
        return Err(PersistenceError::InvalidRecord {
            field: "storage_location_path",
        });
    }
    if location.updated_at_unix_ms < 0 {
        return Err(PersistenceError::InvalidRecord {
            field: "storage_location_updated_at",
        });
    }
    Ok(())
}

fn validate_text(field: &'static str, value: &str, limit: usize) -> Result<(), PersistenceError> {
    if value.is_empty() || value.len() > limit || value.contains('\0') {
        return Err(PersistenceError::InvalidRecord { field });
    }
    Ok(())
}
