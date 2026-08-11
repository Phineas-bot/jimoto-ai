use std::fmt;

use gixgiz_contracts::{CorrelationId, RuntimeConsentState, RuntimeOwnership, RuntimeProviderId};
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

use crate::{Persistence, PersistenceError};

const KEY_BYTES_MAX: usize = 128;
const VALUE_BYTES_MAX: usize = 4096;
const CATEGORY_BYTES_MAX: usize = 64;
const AUDIT_PAGE_MAX: u16 = 1_000;

/// Opaque durable-job identifier with bounded text representation.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct JobId(String);

impl JobId {
    /// Creates and validates a durable-job identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, PersistenceError> {
        let value = value.into();
        validate_text("job_id", &value, KEY_BYTES_MAX)?;
        Ok(Self(value))
    }

    /// Generates a random durable-job identifier without embedding user data.
    #[must_use]
    pub fn generated() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Returns the bounded identifier text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Minimal durable lifecycle state for future restart-safe jobs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum JobState {
    /// Work is durably queued but has not started.
    Queued,
    /// Work is currently running.
    Running,
    /// Work completed successfully.
    Succeeded,
    /// Work ended in failure.
    Failed,
    /// Work observed cooperative cancellation.
    Cancelled,
}

impl JobState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    fn parse(value: &str) -> Result<Self, PersistenceError> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(PersistenceError::InvalidRecord { field: "job_state" }),
        }
    }
}

/// Bounded durable metadata for one future long-running job.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobMetadata {
    /// Opaque stable job identifier.
    pub id: JobId,
    /// Provider-neutral job kind.
    pub kind: String,
    /// Current durable lifecycle state.
    pub state: JobState,
    /// Caller-supplied UTC Unix timestamp in milliseconds.
    pub updated_at_unix_ms: i64,
}

impl JobMetadata {
    /// Creates validated job metadata without provider payloads or private data.
    pub fn new(
        id: JobId,
        kind: impl Into<String>,
        state: JobState,
        updated_at_unix_ms: i64,
    ) -> Result<Self, PersistenceError> {
        let kind = kind.into();
        validate_text("job_kind", &kind, KEY_BYTES_MAX)?;
        validate_timestamp(updated_at_unix_ms)?;
        Ok(Self {
            id,
            kind,
            state,
            updated_at_unix_ms,
        })
    }
}

/// Opaque identifier for an append-only audit event.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AuditEventId(Uuid);

impl AuditEventId {
    fn generated() -> Self {
        Self(Uuid::new_v4())
    }

    fn parse(value: &str) -> Result<Self, PersistenceError> {
        Uuid::parse_str(value)
            .map(Self)
            .map_err(|_| PersistenceError::InvalidRecord {
                field: "audit_event_id",
            })
    }
}

impl fmt::Display for AuditEventId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Redaction-safe audit event containing categorical metadata only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditEvent {
    /// Opaque event identifier.
    pub id: AuditEventId,
    /// Stable event category.
    pub category: String,
    /// Stable action name.
    pub action: String,
    /// Stable outcome name.
    pub outcome: String,
    /// Correlation identifier shared with structured logs and operations.
    pub correlation_id: CorrelationId,
    /// Caller-supplied UTC Unix timestamp in milliseconds.
    pub occurred_at_unix_ms: i64,
}

/// Durable provider-neutral ownership and consent policy for one runtime.
///
/// Detection evidence is intentionally absent: observing a runtime must not
/// create, replace, or otherwise mutate this record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePolicyRecord {
    /// Stable provider identity without provider-specific configuration.
    pub provider_id: RuntimeProviderId,
    /// Installation ownership recorded independently from detection.
    pub ownership: RuntimeOwnership,
    /// Explicit decision about reusing an externally managed runtime.
    pub reuse_consent: RuntimeConsentState,
    /// Explicit decision about lifecycle management authority.
    pub management_consent: RuntimeConsentState,
    /// Caller-supplied UTC Unix timestamp in milliseconds.
    pub updated_at_unix_ms: i64,
}

impl RuntimePolicyRecord {
    /// Creates a validated policy record containing no paths or provider data.
    pub fn new(
        provider_id: RuntimeProviderId,
        ownership: RuntimeOwnership,
        reuse_consent: RuntimeConsentState,
        management_consent: RuntimeConsentState,
        updated_at_unix_ms: i64,
    ) -> Result<Self, PersistenceError> {
        validate_text("runtime_provider_id", provider_id.as_str(), KEY_BYTES_MAX)?;
        validate_runtime_ownership(ownership)?;
        validate_reuse_consent(reuse_consent)?;
        validate_management_consent(management_consent)?;
        validate_timestamp(updated_at_unix_ms)?;
        Ok(Self {
            provider_id,
            ownership,
            reuse_consent,
            management_consent,
            updated_at_unix_ms,
        })
    }
}

/// Repository for provider-neutral runtime ownership and consent policy.
#[derive(Clone)]
pub struct RuntimePolicyRepository {
    persistence: Persistence,
}

impl RuntimePolicyRepository {
    pub(crate) fn new(persistence: Persistence) -> Self {
        Self { persistence }
    }

    /// Creates or replaces one policy record in a single transaction.
    pub fn upsert(&self, policy: &RuntimePolicyRecord) -> Result<(), PersistenceError> {
        let validated = RuntimePolicyRecord::new(
            policy.provider_id.clone(),
            policy.ownership,
            policy.reuse_consent,
            policy.management_consent,
            policy.updated_at_unix_ms,
        )?;
        self.persistence.try_with_write_transaction(|transaction| {
            let mut statement = transaction
                .prepare(
                    "INSERT INTO runtime_policy
                     (provider_id, ownership, reuse_consent, management_consent, updated_at_unix_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(provider_id) DO UPDATE SET
                         ownership = excluded.ownership,
                         reuse_consent = excluded.reuse_consent,
                         management_consent = excluded.management_consent,
                         updated_at_unix_ms = excluded.updated_at_unix_ms",
                )
                .map_err(|source| {
                    PersistenceError::sqlite("prepare_runtime_policy_write", source)
                })?;
            statement
                .execute(params![
                    validated.provider_id.as_str(),
                    runtime_ownership_as_str(validated.ownership)?,
                    runtime_consent_as_str(validated.reuse_consent),
                    runtime_consent_as_str(validated.management_consent),
                    validated.updated_at_unix_ms,
                ])
                .map_err(|source| PersistenceError::sqlite("write_runtime_policy", source))?;
            Ok(())
        })
    }

    /// Reads one runtime policy without creating detection or ownership state.
    pub fn get(
        &self,
        provider_id: &RuntimeProviderId,
    ) -> Result<Option<RuntimePolicyRecord>, PersistenceError> {
        validate_text("runtime_provider_id", provider_id.as_str(), KEY_BYTES_MAX)?;
        self.persistence.try_with_connection(|connection| {
            let mut statement = connection
                .prepare(
                    "SELECT provider_id, ownership, reuse_consent, management_consent,
                            updated_at_unix_ms
                     FROM runtime_policy WHERE provider_id = ?1",
                )
                .map_err(|source| {
                    PersistenceError::sqlite("prepare_runtime_policy_read", source)
                })?;
            let row = statement
                .query_row(params![provider_id.as_str()], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                })
                .optional()
                .map_err(|source| PersistenceError::sqlite("read_runtime_policy", source))?;
            row.map(
                |(provider_id, ownership, reuse_consent, management_consent, updated)| {
                    RuntimePolicyRecord::new(
                        RuntimeProviderId::new(provider_id),
                        parse_runtime_ownership(&ownership)?,
                        parse_reuse_consent(&reuse_consent)?,
                        parse_management_consent(&management_consent)?,
                        updated,
                    )
                },
            )
            .transpose()
        })
    }
}

/// Repository for bounded, non-secret platform metadata.
#[derive(Clone)]
pub struct PlatformMetadataRepository {
    persistence: Persistence,
}

impl PlatformMetadataRepository {
    pub(crate) fn new(persistence: Persistence) -> Self {
        Self { persistence }
    }

    /// Creates or replaces a bounded text value in one transaction.
    pub fn set(
        &self,
        key: &str,
        value: &str,
        updated_at_unix_ms: i64,
    ) -> Result<(), PersistenceError> {
        validate_storage_pair(key, value, updated_at_unix_ms)?;
        self.persistence.with_write_transaction(|transaction| {
            let mut statement = transaction
                .prepare(
                    "INSERT INTO application_metadata (key, value, updated_at_unix_ms)
                     VALUES (?1, ?2, ?3)
                     ON CONFLICT(key) DO UPDATE SET
                         value = excluded.value,
                         updated_at_unix_ms = excluded.updated_at_unix_ms",
                )
                .map_err(|source| PersistenceError::sqlite("prepare_metadata_write", source))?;
            statement
                .execute(params![key, value, updated_at_unix_ms])
                .map_err(|source| PersistenceError::sqlite("write_metadata", source))?;
            Ok(())
        })
    }

    /// Reads one bounded metadata value.
    pub fn get(&self, key: &str) -> Result<Option<String>, PersistenceError> {
        validate_key(key)?;
        self.persistence.with_connection(|connection| {
            let mut statement = connection
                .prepare("SELECT value FROM application_metadata WHERE key = ?1")
                .map_err(|source| PersistenceError::sqlite("prepare_metadata_read", source))?;
            statement
                .query_row(params![key], |row| row.get(0))
                .optional()
                .map_err(|source| PersistenceError::sqlite("read_metadata", source))
        })
    }
}

/// Repository for bounded non-secret settings metadata.
#[derive(Clone)]
pub struct SettingsRepository {
    persistence: Persistence,
}

impl SettingsRepository {
    pub(crate) fn new(persistence: Persistence) -> Self {
        Self { persistence }
    }

    /// Creates or replaces a non-secret bounded text setting transactionally.
    pub fn set(
        &self,
        key: &str,
        value: &str,
        updated_at_unix_ms: i64,
    ) -> Result<(), PersistenceError> {
        validate_storage_pair(key, value, updated_at_unix_ms)?;
        self.persistence.with_write_transaction(|transaction| {
            let mut statement = transaction
                .prepare(
                    "INSERT INTO settings (key, value, updated_at_unix_ms)
                     VALUES (?1, ?2, ?3)
                     ON CONFLICT(key) DO UPDATE SET
                         value = excluded.value,
                         updated_at_unix_ms = excluded.updated_at_unix_ms",
                )
                .map_err(|source| PersistenceError::sqlite("prepare_setting_write", source))?;
            statement
                .execute(params![key, value, updated_at_unix_ms])
                .map_err(|source| PersistenceError::sqlite("write_setting", source))?;
            Ok(())
        })
    }

    /// Reads one non-secret setting value.
    pub fn get(&self, key: &str) -> Result<Option<String>, PersistenceError> {
        validate_key(key)?;
        self.persistence.with_connection(|connection| {
            let mut statement = connection
                .prepare("SELECT value FROM settings WHERE key = ?1")
                .map_err(|source| PersistenceError::sqlite("prepare_setting_read", source))?;
            statement
                .query_row(params![key], |row| row.get(0))
                .optional()
                .map_err(|source| PersistenceError::sqlite("read_setting", source))
        })
    }
}

/// Repository for minimal durable job metadata.
#[derive(Clone)]
pub struct JobMetadataRepository {
    persistence: Persistence,
}

impl JobMetadataRepository {
    pub(crate) fn new(persistence: Persistence) -> Self {
        Self { persistence }
    }

    /// Creates or updates one job record transactionally.
    pub fn upsert(&self, job: &JobMetadata) -> Result<(), PersistenceError> {
        let validated = JobMetadata::new(
            job.id.clone(),
            job.kind.clone(),
            job.state,
            job.updated_at_unix_ms,
        )?;
        self.persistence.with_write_transaction(|transaction| {
            let mut statement = transaction
                .prepare(
                    "INSERT INTO durable_jobs (id, kind, state, updated_at_unix_ms)
                     VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(id) DO UPDATE SET
                         kind = excluded.kind,
                         state = excluded.state,
                         updated_at_unix_ms = excluded.updated_at_unix_ms",
                )
                .map_err(|source| PersistenceError::sqlite("prepare_job_write", source))?;
            statement
                .execute(params![
                    validated.id.as_str(),
                    validated.kind,
                    validated.state.as_str(),
                    validated.updated_at_unix_ms,
                ])
                .map_err(|source| PersistenceError::sqlite("write_job", source))?;
            Ok(())
        })
    }

    /// Reads one durable job record.
    pub fn get(&self, id: &JobId) -> Result<Option<JobMetadata>, PersistenceError> {
        self.persistence.with_connection(|connection| {
            let mut statement = connection
                .prepare(
                    "SELECT id, kind, state, updated_at_unix_ms FROM durable_jobs WHERE id = ?1",
                )
                .map_err(|source| PersistenceError::sqlite("prepare_job_read", source))?;
            let row = statement
                .query_row(params![id.as_str()], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })
                .optional()
                .map_err(|source| PersistenceError::sqlite("read_job", source))?;
            row.map(|(id, kind, state, updated)| {
                JobMetadata::new(JobId::new(id)?, kind, JobState::parse(&state)?, updated)
            })
            .transpose()
        })
    }
}

/// Append-only repository for redaction-safe audit events.
#[derive(Clone)]
pub struct AuditEventRepository {
    persistence: Persistence,
}

impl AuditEventRepository {
    pub(crate) fn new(persistence: Persistence) -> Self {
        Self { persistence }
    }

    /// Appends one categorical event without accepting message or payload data.
    pub fn append(
        &self,
        category: &str,
        action: &str,
        outcome: &str,
        correlation_id: CorrelationId,
        occurred_at_unix_ms: i64,
    ) -> Result<AuditEvent, PersistenceError> {
        validate_text("audit_category", category, CATEGORY_BYTES_MAX)?;
        validate_text("audit_action", action, KEY_BYTES_MAX)?;
        validate_text("audit_outcome", outcome, CATEGORY_BYTES_MAX)?;
        validate_timestamp(occurred_at_unix_ms)?;
        let event = AuditEvent {
            id: AuditEventId::generated(),
            category: category.to_owned(),
            action: action.to_owned(),
            outcome: outcome.to_owned(),
            correlation_id,
            occurred_at_unix_ms,
        };
        self.persistence.with_write_transaction(|transaction| {
            let mut statement = transaction
                .prepare(
                    "INSERT INTO audit_events
                     (id, category, action, outcome, correlation_id, occurred_at_unix_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )
                .map_err(|source| PersistenceError::sqlite("prepare_audit_write", source))?;
            statement
                .execute(params![
                    event.id.to_string(),
                    event.category,
                    event.action,
                    event.outcome,
                    event.correlation_id.to_string(),
                    event.occurred_at_unix_ms,
                ])
                .map_err(|source| PersistenceError::sqlite("write_audit_event", source))?;
            Ok(())
        })?;
        Ok(event)
    }

    /// Lists a bounded page of newest audit events in deterministic order.
    pub fn list_recent(&self, limit: u16) -> Result<Vec<AuditEvent>, PersistenceError> {
        if limit == 0 || limit > AUDIT_PAGE_MAX {
            return Err(PersistenceError::InvalidRecord {
                field: "audit_limit",
            });
        }
        self.persistence.with_connection(|connection| {
            let mut statement = connection
                .prepare(
                    "SELECT id, category, action, outcome, correlation_id, occurred_at_unix_ms
                     FROM audit_events ORDER BY occurred_at_unix_ms DESC, id DESC LIMIT ?1",
                )
                .map_err(|source| PersistenceError::sqlite("prepare_audit_read", source))?;
            let rows = statement
                .query_map(params![limit], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                })
                .map_err(|source| PersistenceError::sqlite("read_audit_events", source))?;
            rows.map(|row| {
                let (id, category, action, outcome, correlation, occurred) =
                    row.map_err(|source| PersistenceError::sqlite("read_audit_event", source))?;
                Ok(AuditEvent {
                    id: AuditEventId::parse(&id)?,
                    category,
                    action,
                    outcome,
                    correlation_id: correlation.parse().map_err(|_| {
                        PersistenceError::InvalidRecord {
                            field: "correlation_id",
                        }
                    })?,
                    occurred_at_unix_ms: occurred,
                })
            })
            .collect()
        })
    }
}

fn validate_storage_pair(
    key: &str,
    value: &str,
    updated_at_unix_ms: i64,
) -> Result<(), PersistenceError> {
    validate_key(key)?;
    validate_text("value", value, VALUE_BYTES_MAX)?;
    validate_timestamp(updated_at_unix_ms)
}

fn validate_key(key: &str) -> Result<(), PersistenceError> {
    validate_text("key", key, KEY_BYTES_MAX)?;
    let normalized = key.to_ascii_lowercase();
    if [
        "secret",
        "password",
        "token",
        "credential",
        "private_key",
        "api_key",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
    {
        return Err(PersistenceError::InvalidRecord { field: "key" });
    }
    Ok(())
}

fn validate_text(
    field: &'static str,
    value: &str,
    maximum_bytes: usize,
) -> Result<(), PersistenceError> {
    if value.is_empty() || value.len() > maximum_bytes || value.chars().any(char::is_control) {
        Err(PersistenceError::InvalidRecord { field })
    } else {
        Ok(())
    }
}

fn validate_timestamp(value: i64) -> Result<(), PersistenceError> {
    if value < 0 {
        Err(PersistenceError::InvalidRecord { field: "timestamp" })
    } else {
        Ok(())
    }
}

fn validate_runtime_ownership(value: RuntimeOwnership) -> Result<(), PersistenceError> {
    match value {
        RuntimeOwnership::External
        | RuntimeOwnership::GixGizManaged
        | RuntimeOwnership::Bundled
        | RuntimeOwnership::Unknown => Ok(()),
        _ => Err(PersistenceError::InvalidRecord {
            field: "runtime_ownership",
        }),
    }
}

fn runtime_ownership_as_str(value: RuntimeOwnership) -> Result<&'static str, PersistenceError> {
    match value {
        RuntimeOwnership::External => Ok("external"),
        RuntimeOwnership::GixGizManaged => Ok("gix_giz_managed"),
        RuntimeOwnership::Bundled => Ok("bundled"),
        RuntimeOwnership::Unknown => Ok("unknown"),
        _ => Err(PersistenceError::InvalidRecord {
            field: "runtime_ownership",
        }),
    }
}

fn parse_runtime_ownership(value: &str) -> Result<RuntimeOwnership, PersistenceError> {
    match value {
        "external" => Ok(RuntimeOwnership::External),
        "gix_giz_managed" => Ok(RuntimeOwnership::GixGizManaged),
        "bundled" => Ok(RuntimeOwnership::Bundled),
        "unknown" => Ok(RuntimeOwnership::Unknown),
        _ => Err(PersistenceError::InvalidRecord {
            field: "runtime_ownership",
        }),
    }
}

const fn runtime_consent_as_str(value: RuntimeConsentState) -> &'static str {
    match value {
        RuntimeConsentState::NotRequested => "not_requested",
        RuntimeConsentState::ReuseApproved => "reuse_approved",
        RuntimeConsentState::ManagementApproved => "management_approved",
        RuntimeConsentState::Denied => "denied",
        RuntimeConsentState::Unknown => "unknown",
        _ => "unknown",
    }
}

fn validate_reuse_consent(value: RuntimeConsentState) -> Result<(), PersistenceError> {
    match value {
        RuntimeConsentState::NotRequested
        | RuntimeConsentState::ReuseApproved
        | RuntimeConsentState::Denied
        | RuntimeConsentState::Unknown => Ok(()),
        RuntimeConsentState::ManagementApproved => Err(PersistenceError::InvalidRecord {
            field: "runtime_reuse_consent",
        }),
        _ => Err(PersistenceError::InvalidRecord {
            field: "runtime_reuse_consent",
        }),
    }
}

fn validate_management_consent(value: RuntimeConsentState) -> Result<(), PersistenceError> {
    match value {
        RuntimeConsentState::NotRequested
        | RuntimeConsentState::ManagementApproved
        | RuntimeConsentState::Denied
        | RuntimeConsentState::Unknown => Ok(()),
        RuntimeConsentState::ReuseApproved => Err(PersistenceError::InvalidRecord {
            field: "runtime_management_consent",
        }),
        _ => Err(PersistenceError::InvalidRecord {
            field: "runtime_management_consent",
        }),
    }
}

fn parse_reuse_consent(value: &str) -> Result<RuntimeConsentState, PersistenceError> {
    match value {
        "not_requested" => Ok(RuntimeConsentState::NotRequested),
        "reuse_approved" => Ok(RuntimeConsentState::ReuseApproved),
        "denied" => Ok(RuntimeConsentState::Denied),
        "unknown" => Ok(RuntimeConsentState::Unknown),
        _ => Err(PersistenceError::InvalidRecord {
            field: "runtime_reuse_consent",
        }),
    }
}

fn parse_management_consent(value: &str) -> Result<RuntimeConsentState, PersistenceError> {
    match value {
        "not_requested" => Ok(RuntimeConsentState::NotRequested),
        "management_approved" => Ok(RuntimeConsentState::ManagementApproved),
        "denied" => Ok(RuntimeConsentState::Denied),
        "unknown" => Ok(RuntimeConsentState::Unknown),
        _ => Err(PersistenceError::InvalidRecord {
            field: "runtime_management_consent",
        }),
    }
}
