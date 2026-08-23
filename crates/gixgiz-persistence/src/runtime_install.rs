//! Durable storage for approved managed runtime installation.
//!
//! SQLite is authoritative for every installation transition. This module
//! stores categorical metadata and bounded text only: never the installer
//! binary, raw process output, credentials, full command lines, or private
//! filesystem paths.

use rusqlite::{OptionalExtension, params};

use crate::{Persistence, PersistenceError};

const ID_BYTES_MAX: usize = 64;
const ORIGIN_BYTES_MAX: usize = 512;
const NAME_BYTES_MAX: usize = 256;
const CATEGORY_BYTES_MAX: usize = 64;
const PROVIDER_BYTES_MAX: usize = 128;
const JSON_BYTES_MAX: usize = 4096;
const VERIFICATION_BYTES_MAX: usize = 512;

/// Durable runtime-installation record owned by Rust.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedRuntimeInstallJob {
    /// Opaque job identifier.
    pub job_id: String,
    /// Provider this job installs.
    pub provider_id: String,
    /// Current immutable plan revision.
    pub plan_revision: u32,
    /// Exact version the plan installs.
    pub version: String,
    /// Bounded display origin without query parameters.
    pub source_origin: String,
    /// Bounded artifact file name.
    pub artifact_name: String,
    /// Conservative expected transfer size.
    pub expected_size_bytes: u64,
    /// Whether the plan expected an administrator prompt.
    pub requires_administrator: bool,
    /// Durable state.
    pub state: String,
    /// Furthest stage reached.
    pub stage: String,
    /// Approved revision when an approval exists.
    pub approved_revision: Option<u32>,
    /// When the approval was recorded.
    pub approved_at_unix_ms: Option<i64>,
    /// Bytes transferred into GixGiz-owned staging.
    pub transferred_bytes: Option<u64>,
    /// Bounded verification evidence as JSON.
    pub verification_json: Option<String>,
    /// Bounded effect report as JSON.
    pub effects_json: String,
    /// Attention reason when a decision is needed.
    pub attention: Option<String>,
    /// Safe failure classification.
    pub failure: Option<String>,
    /// Safe recovery route.
    pub recovery: Option<String>,
    /// Ownership recorded for the provider.
    pub ownership: String,
    /// Retries consumed.
    pub retry_count: u32,
    /// Creation timestamp.
    pub created_at_unix_ms: i64,
    /// Last durable transition timestamp.
    pub updated_at_unix_ms: i64,
}

/// Repository for durable runtime-installation jobs.
#[derive(Clone)]
pub struct RuntimeInstallRepository {
    persistence: Persistence,
}

impl RuntimeInstallRepository {
    pub(crate) fn new(persistence: Persistence) -> Self {
        Self { persistence }
    }

    /// Creates or replaces one installation job in a single transaction.
    pub fn upsert(&self, job: &PersistedRuntimeInstallJob) -> Result<(), PersistenceError> {
        validate(job)?;
        self.persistence.try_with_write_transaction(|transaction| {
            transaction
                .execute(
                    "INSERT INTO runtime_install_jobs
                     (job_id, provider_id, plan_revision, version, source_origin, artifact_name,
                      expected_size_bytes, requires_administrator, state, stage, approved_revision,
                      approved_at_unix_ms, transferred_bytes, verification_json, effects_json,
                      attention, failure, recovery, ownership, retry_count,
                      created_at_unix_ms, updated_at_unix_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                             ?16, ?17, ?18, ?19, ?20, ?21, ?22)
                     ON CONFLICT(job_id) DO UPDATE SET
                         plan_revision = excluded.plan_revision,
                         state = excluded.state,
                         stage = excluded.stage,
                         approved_revision = excluded.approved_revision,
                         approved_at_unix_ms = excluded.approved_at_unix_ms,
                         transferred_bytes = excluded.transferred_bytes,
                         verification_json = excluded.verification_json,
                         effects_json = excluded.effects_json,
                         attention = excluded.attention,
                         failure = excluded.failure,
                         recovery = excluded.recovery,
                         ownership = excluded.ownership,
                         retry_count = excluded.retry_count,
                         updated_at_unix_ms = excluded.updated_at_unix_ms",
                    params![
                        job.job_id,
                        job.provider_id,
                        job.plan_revision,
                        job.version,
                        job.source_origin,
                        job.artifact_name,
                        job.expected_size_bytes as i64,
                        i32::from(job.requires_administrator),
                        job.state,
                        job.stage,
                        job.approved_revision,
                        job.approved_at_unix_ms,
                        job.transferred_bytes.map(|value| value as i64),
                        job.verification_json,
                        job.effects_json,
                        job.attention,
                        job.failure,
                        job.recovery,
                        job.ownership,
                        job.retry_count,
                        job.created_at_unix_ms,
                        job.updated_at_unix_ms,
                    ],
                )
                .map_err(|source| PersistenceError::sqlite("write_runtime_install_job", source))?;
            Ok(())
        })
    }

    /// Reads one installation job.
    pub fn get(
        &self,
        job_id: &str,
    ) -> Result<Option<PersistedRuntimeInstallJob>, PersistenceError> {
        validate_text("runtime_install_job_id", job_id, ID_BYTES_MAX)?;
        self.persistence.try_with_connection(|connection| {
            connection
                .query_row(
                    &format!("SELECT {COLUMNS} FROM runtime_install_jobs WHERE job_id = ?1"),
                    params![job_id],
                    decode,
                )
                .optional()
                .map_err(|source| PersistenceError::sqlite("read_runtime_install_job", source))
        })
    }

    /// Returns the single non-terminal job for a provider, when one exists.
    ///
    /// Used at startup to reconcile a job interrupted by an earlier exit.
    pub fn active(
        &self,
        provider_id: &str,
    ) -> Result<Option<PersistedRuntimeInstallJob>, PersistenceError> {
        validate_text("runtime_provider_id", provider_id, PROVIDER_BYTES_MAX)?;
        self.persistence.try_with_connection(|connection| {
            connection
                .query_row(
                    &format!(
                        "SELECT {COLUMNS} FROM runtime_install_jobs
                         WHERE provider_id = ?1
                           AND state NOT IN ('ready', 'failed', 'cancelled')"
                    ),
                    params![provider_id],
                    decode,
                )
                .optional()
                .map_err(|source| {
                    PersistenceError::sqlite("read_active_runtime_install_job", source)
                })
        })
    }
}

const COLUMNS: &str = "job_id, provider_id, plan_revision, version, source_origin, artifact_name, \
     expected_size_bytes, requires_administrator, state, stage, approved_revision, \
     approved_at_unix_ms, transferred_bytes, verification_json, effects_json, attention, \
     failure, recovery, ownership, retry_count, created_at_unix_ms, updated_at_unix_ms";

fn decode(row: &rusqlite::Row<'_>) -> rusqlite::Result<PersistedRuntimeInstallJob> {
    Ok(PersistedRuntimeInstallJob {
        job_id: row.get(0)?,
        provider_id: row.get(1)?,
        plan_revision: row.get(2)?,
        version: row.get(3)?,
        source_origin: row.get(4)?,
        artifact_name: row.get(5)?,
        expected_size_bytes: row.get::<_, i64>(6)?.max(0) as u64,
        requires_administrator: row.get::<_, i32>(7)? != 0,
        state: row.get(8)?,
        stage: row.get(9)?,
        approved_revision: row.get(10)?,
        approved_at_unix_ms: row.get(11)?,
        transferred_bytes: row
            .get::<_, Option<i64>>(12)?
            .map(|value| value.max(0) as u64),
        verification_json: row.get(13)?,
        effects_json: row.get(14)?,
        attention: row.get(15)?,
        failure: row.get(16)?,
        recovery: row.get(17)?,
        ownership: row.get(18)?,
        retry_count: row.get(19)?,
        created_at_unix_ms: row.get(20)?,
        updated_at_unix_ms: row.get(21)?,
    })
}

fn validate(job: &PersistedRuntimeInstallJob) -> Result<(), PersistenceError> {
    validate_text("runtime_install_job_id", &job.job_id, ID_BYTES_MAX)?;
    validate_text("runtime_provider_id", &job.provider_id, PROVIDER_BYTES_MAX)?;
    validate_text("runtime_install_version", &job.version, CATEGORY_BYTES_MAX)?;
    validate_text(
        "runtime_install_source_origin",
        &job.source_origin,
        ORIGIN_BYTES_MAX,
    )?;
    validate_text(
        "runtime_install_artifact_name",
        &job.artifact_name,
        NAME_BYTES_MAX,
    )?;
    validate_text("runtime_install_state", &job.state, CATEGORY_BYTES_MAX)?;
    validate_text("runtime_install_stage", &job.stage, CATEGORY_BYTES_MAX)?;
    validate_text(
        "runtime_install_ownership",
        &job.ownership,
        CATEGORY_BYTES_MAX,
    )?;
    validate_text("runtime_install_effects", &job.effects_json, JSON_BYTES_MAX)?;
    if let Some(value) = job.verification_json.as_deref() {
        validate_text(
            "runtime_install_verification",
            value,
            VERIFICATION_BYTES_MAX,
        )?;
    }
    for (field, value) in [
        ("runtime_install_attention", job.attention.as_deref()),
        ("runtime_install_failure", job.failure.as_deref()),
        ("runtime_install_recovery", job.recovery.as_deref()),
    ] {
        if let Some(value) = value {
            validate_text(field, value, CATEGORY_BYTES_MAX)?;
        }
    }
    if job.plan_revision == 0 {
        return Err(PersistenceError::InvalidRecord {
            field: "runtime_install_plan_revision",
        });
    }
    if job.created_at_unix_ms < 0 || job.updated_at_unix_ms < 0 {
        return Err(PersistenceError::InvalidRecord {
            field: "runtime_install_timestamp",
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
