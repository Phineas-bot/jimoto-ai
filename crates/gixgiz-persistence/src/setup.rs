//! Bounded persistence records for the durable model-setup workflow.

use std::collections::HashSet;

use gixgiz_contracts::{
    AccelerationKind, CandidateModelId, CatalogueVersion, CorrelationId, ModelAcquisitionPhase,
    ModelSizeClass, RequestId, RuleSetVersion, RuntimeProviderId, RuntimeProviderModelId,
};
use rusqlite::{OptionalExtension, Row, params};

use crate::{JobId, Persistence, PersistenceError};

const ID_BYTES_MAX: usize = 128;
const PROVIDER_MODEL_ID_BYTES_MAX: usize = 256;
const DISPLAY_BYTES_MAX: usize = 256;
const URL_BYTES_MAX: usize = 2_048;
const NOTICE_BYTES_MAX: usize = 512;
const NOTICE_COUNT_MAX: usize = 64;
const EFFECT_COUNT_MAX: usize = 64;
const EVENT_PAGE_MAX: u16 = 256;
const EVENT_RETENTION_MAX: u16 = 256;

/// Fine-grained authoritative state of one model-setup job.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PersistedSetupState {
    /// A concrete plan is being assembled without external effects.
    DraftPlan,
    /// The immutable plan is waiting for exact user approval.
    AwaitingApproval,
    /// Approval is durable and the worker has not started provider work.
    Approved,
    /// Current runtime and policy preconditions are being checked.
    Preparing,
    /// The actual destination is being checked for availability and space.
    CheckingStorage,
    /// The selected provider artifact is being acquired.
    Acquiring,
    /// Provider registration is being verified.
    Registering,
    /// Runtime health is being reverified after acquisition.
    VerifyingRuntime,
    /// Exact model availability is being verified.
    VerifyingModel,
    /// A fixed bounded readiness inference is running.
    RunningTestInference,
    /// Every setup-readiness gate succeeded.
    Ready,
    /// Recoverable user or environment action is required.
    AttentionRequired,
    /// The job reached a non-recoverable attempt failure.
    Failed,
    /// Cooperative cancellation completed with a structured effect report.
    Cancelled,
}

impl PersistedSetupState {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::DraftPlan => "draft_plan",
            Self::AwaitingApproval => "awaiting_approval",
            Self::Approved => "approved",
            Self::Preparing => "preparing",
            Self::CheckingStorage => "checking_storage",
            Self::Acquiring => "acquiring",
            Self::Registering => "registering",
            Self::VerifyingRuntime => "verifying_runtime",
            Self::VerifyingModel => "verifying_model",
            Self::RunningTestInference => "running_test_inference",
            Self::Ready => "ready",
            Self::AttentionRequired => "attention_required",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    fn parse(value: &str) -> Result<Self, PersistenceError> {
        match value {
            "draft_plan" => Ok(Self::DraftPlan),
            "awaiting_approval" => Ok(Self::AwaitingApproval),
            "approved" => Ok(Self::Approved),
            "preparing" => Ok(Self::Preparing),
            "checking_storage" => Ok(Self::CheckingStorage),
            "acquiring" => Ok(Self::Acquiring),
            "registering" => Ok(Self::Registering),
            "verifying_runtime" => Ok(Self::VerifyingRuntime),
            "verifying_model" => Ok(Self::VerifyingModel),
            "running_test_inference" => Ok(Self::RunningTestInference),
            "ready" => Ok(Self::Ready),
            "attention_required" => Ok(Self::AttentionRequired),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(PersistenceError::InvalidRecord {
                field: "setup_state",
            }),
        }
    }

    /// Returns whether the state is terminal for the current attempt.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Ready | Self::Failed | Self::Cancelled)
    }

    /// Returns whether an interrupted state needs startup recovery classification.
    #[must_use]
    pub const fn is_interrupted_work(self) -> bool {
        matches!(
            self,
            Self::Preparing
                | Self::CheckingStorage
                | Self::Acquiring
                | Self::Registering
                | Self::VerifyingRuntime
                | Self::VerifyingModel
                | Self::RunningTestInference
        )
    }
}

/// Coarse user-visible stage associated with a setup state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PersistedSetupStage {
    /// Plan assembly or review.
    PlanReview,
    /// Approval review.
    Approval,
    /// Runtime and policy preparation.
    Preparation,
    /// Destination storage checks.
    Storage,
    /// Model acquisition.
    Acquisition,
    /// Provider registration.
    Registration,
    /// Runtime verification.
    RuntimeVerification,
    /// Model verification.
    ModelVerification,
    /// Readiness inference verification.
    InferenceVerification,
    /// Successful completion.
    Complete,
    /// Recoverable attention.
    Attention,
    /// Failed completion.
    Failed,
    /// Cancelled completion.
    Cancelled,
}

impl PersistedSetupStage {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::PlanReview => "plan_review",
            Self::Approval => "approval",
            Self::Preparation => "preparation",
            Self::Storage => "storage",
            Self::Acquisition => "acquisition",
            Self::Registration => "registration",
            Self::RuntimeVerification => "runtime_verification",
            Self::ModelVerification => "model_verification",
            Self::InferenceVerification => "inference_verification",
            Self::Complete => "complete",
            Self::Attention => "attention",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    fn parse(value: &str) -> Result<Self, PersistenceError> {
        match value {
            "plan_review" => Ok(Self::PlanReview),
            "approval" => Ok(Self::Approval),
            "preparation" => Ok(Self::Preparation),
            "storage" => Ok(Self::Storage),
            "acquisition" => Ok(Self::Acquisition),
            "registration" => Ok(Self::Registration),
            "runtime_verification" => Ok(Self::RuntimeVerification),
            "model_verification" => Ok(Self::ModelVerification),
            "inference_verification" => Ok(Self::InferenceVerification),
            "complete" => Ok(Self::Complete),
            "attention" => Ok(Self::Attention),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(PersistenceError::InvalidRecord {
                field: "setup_stage",
            }),
        }
    }
}

/// Privacy-preserving ownership category for model storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PersistedDestinationCategory {
    /// The runtime provider owns the destination and its partial artifacts.
    ProviderManaged,
    /// GixGiz owns an application-data destination.
    ApplicationData,
    /// A separately approved user-selected destination is represented opaquely.
    UserSelected,
}

/// Explicit durable decision for one immutable setup-plan revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PersistedSetupApprovalDecision {
    /// Authorize only the exact persisted plan and effect list.
    Approve,
    /// Deny the plan without authorizing provider effects.
    Deny,
}

impl PersistedSetupApprovalDecision {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Deny => "deny",
        }
    }

    fn parse(value: &str) -> Result<Self, PersistenceError> {
        match value {
            "approve" => Ok(Self::Approve),
            "deny" => Ok(Self::Deny),
            _ => Err(PersistenceError::InvalidRecord {
                field: "setup_approval_decision",
            }),
        }
    }
}

impl PersistedDestinationCategory {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ProviderManaged => "provider_managed",
            Self::ApplicationData => "application_data",
            Self::UserSelected => "user_selected",
        }
    }

    fn parse(value: &str) -> Result<Self, PersistenceError> {
        match value {
            "provider_managed" => Ok(Self::ProviderManaged),
            "application_data" => Ok(Self::ApplicationData),
            "user_selected" => Ok(Self::UserSelected),
            _ => Err(PersistenceError::InvalidRecord {
                field: "destination_category",
            }),
        }
    }
}

/// Durable event category for bounded status replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PersistedSetupEventKind {
    /// Initial or requested authoritative snapshot.
    Snapshot,
    /// A setup stage changed.
    StageChanged,
    /// Coalesced acquisition progress changed.
    Progress,
    /// Cancellation intent became durable.
    CancellationRequested,
    /// A retry was explicitly requested.
    Retry,
    /// Startup recovery classified interrupted work.
    Recovery,
    /// A terminal state was committed.
    Terminal,
}

impl PersistedSetupEventKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Snapshot => "snapshot",
            Self::StageChanged => "stage_changed",
            Self::Progress => "progress",
            Self::CancellationRequested => "cancellation_requested",
            Self::Retry => "retry",
            Self::Recovery => "recovery",
            Self::Terminal => "terminal",
        }
    }

    fn parse(value: &str) -> Result<Self, PersistenceError> {
        match value {
            "snapshot" => Ok(Self::Snapshot),
            "stage_changed" => Ok(Self::StageChanged),
            "progress" => Ok(Self::Progress),
            "cancellation_requested" => Ok(Self::CancellationRequested),
            "retry" => Ok(Self::Retry),
            "recovery" => Ok(Self::Recovery),
            "terminal" => Ok(Self::Terminal),
            _ => Err(PersistenceError::InvalidRecord {
                field: "setup_event_kind",
            }),
        }
    }
}

/// Outcome category for one bounded setup effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PersistedEffectDisposition {
    /// The approved effect completed and was verified.
    Completed,
    /// The effect remains after cancellation or failure.
    Retained,
    /// GixGiz rolled back an operation-owned effect.
    RolledBack,
    /// The provider effect could not be classified safely.
    Uncertain,
}

impl PersistedEffectDisposition {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Retained => "retained",
            Self::RolledBack => "rolled_back",
            Self::Uncertain => "uncertain",
        }
    }

    fn parse(value: &str) -> Result<Self, PersistenceError> {
        match value {
            "completed" => Ok(Self::Completed),
            "retained" => Ok(Self::Retained),
            "rolled_back" => Ok(Self::RolledBack),
            "uncertain" => Ok(Self::Uncertain),
            _ => Err(PersistenceError::InvalidRecord {
                field: "setup_effect_disposition",
            }),
        }
    }
}

/// Persisted lifecycle of one provider artifact mapping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PersistedModelLifecycle {
    /// The artifact is planned but no acquisition started.
    Planned,
    /// Provider-managed acquisition is active.
    Acquiring,
    /// Acquisition completed but readiness is unverified.
    AcquiredUnverified,
    /// Registration, availability, or inference is being verified.
    Verifying,
    /// All Task 10 readiness gates succeeded.
    Available,
    /// A provider-owned artifact may remain but is not available to GixGiz.
    RetainedUnverified,
    /// Available identity or integrity evidence was rejected.
    Rejected,
    /// Artifact preparation failed without a verified available result.
    Failed,
}

impl PersistedModelLifecycle {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Acquiring => "acquiring",
            Self::AcquiredUnverified => "acquired_unverified",
            Self::Verifying => "verifying",
            Self::Available => "available",
            Self::RetainedUnverified => "retained_unverified",
            Self::Rejected => "rejected",
            Self::Failed => "failed",
        }
    }

    fn parse(value: &str) -> Result<Self, PersistenceError> {
        match value {
            "planned" => Ok(Self::Planned),
            "acquiring" => Ok(Self::Acquiring),
            "acquired_unverified" => Ok(Self::AcquiredUnverified),
            "verifying" => Ok(Self::Verifying),
            "available" => Ok(Self::Available),
            "retained_unverified" => Ok(Self::RetainedUnverified),
            "rejected" => Ok(Self::Rejected),
            "failed" => Ok(Self::Failed),
            _ => Err(PersistenceError::InvalidRecord {
                field: "model_lifecycle_state",
            }),
        }
    }
}

/// Aggregate verification state for one provider artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PersistedModelVerification {
    /// Verification has not started.
    NotStarted,
    /// At least one readiness gate remains incomplete.
    Pending,
    /// Every readiness gate succeeded.
    Verified,
    /// A readiness gate failed.
    Failed,
    /// Evidence is unavailable after interruption.
    Unknown,
}

impl PersistedModelVerification {
    const fn as_str(self) -> &'static str {
        match self {
            Self::NotStarted => "not_started",
            Self::Pending => "pending",
            Self::Verified => "verified",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }

    fn parse(value: &str) -> Result<Self, PersistenceError> {
        match value {
            "not_started" => Ok(Self::NotStarted),
            "pending" => Ok(Self::Pending),
            "verified" => Ok(Self::Verified),
            "failed" => Ok(Self::Failed),
            "unknown" => Ok(Self::Unknown),
            _ => Err(PersistenceError::InvalidRecord {
                field: "model_verification_state",
            }),
        }
    }
}

/// Available integrity evidence for one provider artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PersistedModelIntegrity {
    /// The provider source exposes no expected integrity value.
    Unavailable,
    /// The provider reported integrity metadata without an expected trusted value.
    ProviderReported,
    /// Trusted expected metadata matched.
    Verified,
    /// Trusted expected metadata mismatched and the artifact was rejected.
    Mismatch,
    /// Evidence became uncertain after interruption.
    Unknown,
}

impl PersistedModelIntegrity {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::ProviderReported => "provider_reported",
            Self::Verified => "verified",
            Self::Mismatch => "mismatch",
            Self::Unknown => "unknown",
        }
    }

    fn parse(value: &str) -> Result<Self, PersistenceError> {
        match value {
            "unavailable" => Ok(Self::Unavailable),
            "provider_reported" => Ok(Self::ProviderReported),
            "verified" => Ok(Self::Verified),
            "mismatch" => Ok(Self::Mismatch),
            "unknown" => Ok(Self::Unknown),
            _ => Err(PersistenceError::InvalidRecord {
                field: "model_integrity_state",
            }),
        }
    }
}

/// Latest bounded acquisition counters persisted for restart recovery.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PersistedSetupProgress {
    /// Provider-normalized phase when acquisition progress is available.
    pub phase: Option<ModelAcquisitionPhase>,
    /// Provider-normalized completed bytes when available.
    pub completed_bytes: Option<u64>,
    /// Provider-normalized total bytes when available.
    pub total_bytes: Option<u64>,
    /// Provider-normalized progress in basis points when available.
    pub basis_points: Option<u16>,
}

impl PersistedSetupProgress {
    fn validate(self) -> Result<Self, PersistenceError> {
        if self
            .phase
            .is_some_and(|phase| matches!(phase, ModelAcquisitionPhase::Unknown))
            || self.basis_points.is_some_and(|value| value > 10_000)
            || matches!((self.completed_bytes, self.total_bytes), (Some(done), Some(total)) if done > total)
            || self
                .completed_bytes
                .is_some_and(|value| value > i64::MAX as u64)
            || self
                .total_bytes
                .is_some_and(|value| value > i64::MAX as u64)
        {
            return Err(PersistenceError::InvalidRecord {
                field: "setup_progress",
            });
        }
        Ok(self)
    }
}

/// One bounded reason or warning retained with the immutable setup plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedSetupNotice {
    /// Whether this entry is a reason or warning.
    pub warning: bool,
    /// Stable machine-readable notice code.
    pub code: String,
    /// Safe plain-language message containing no provider payload.
    pub message: String,
}

/// Immutable provider-neutral plan snapshot used to create a setup job.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedSetupPlan {
    /// Opaque durable job identifier.
    pub job_id: JobId,
    /// Setup contract schema generation.
    pub plan_schema_version: u32,
    /// Immutable user-visible plan revision bound to an approval decision.
    pub plan_revision: u32,
    /// Stable canonical catalogue model identity.
    pub canonical_model_id: CandidateModelId,
    /// Beginner-readable model name.
    pub model_display_name: String,
    /// Canonical model family.
    pub model_family: String,
    /// Coarse catalogue size class.
    pub model_size_class: ModelSizeClass,
    /// SPDX licence identifier.
    pub licence_spdx: String,
    /// Official HTTPS provenance URL.
    pub provenance_url: String,
    /// Runtime provider selected by composition.
    pub provider_id: RuntimeProviderId,
    /// Opaque adapter-normalized provider model identifier.
    pub provider_model_id: RuntimeProviderModelId,
    /// Bounded provider-neutral artifact source summary.
    pub artifact_source_summary: String,
    /// Bounded destination label containing no private local path.
    pub destination_display: String,
    /// Bounded provider display name.
    pub runtime_display_name: String,
    /// Verified provider version when known.
    pub runtime_version: Option<String>,
    /// Catalogue version that produced the recommendation.
    pub catalogue_version: CatalogueVersion,
    /// Rule-set version that produced the recommendation.
    pub rule_set_version: RuleSetVersion,
    /// Privacy-preserving destination category.
    pub destination_category: PersistedDestinationCategory,
    /// Conservative expected artifact size.
    pub expected_size_bytes: u64,
    /// Conservative expected working-memory use.
    pub expected_memory_bytes: u64,
    /// Memory headroom retained by the recommendation.
    pub memory_safety_margin_bytes: u64,
    /// Storage headroom retained before acquisition.
    pub storage_safety_margin_bytes: u64,
    /// Planned readiness context bound.
    pub planned_context_tokens: u32,
    /// Whether the plan remains usable without acceleration.
    pub cpu_only: bool,
    /// Reliably evidenced GPU memory selected by the plan.
    pub gpu_memory_bytes: Option<u64>,
    /// Observed total physical memory copied from the recommendation.
    pub observed_total_memory_bytes: Option<u64>,
    /// Observed available physical memory copied from the recommendation.
    pub observed_available_memory_bytes: Option<u64>,
    /// Observed free storage copied from the recommendation.
    pub observed_free_storage_bytes: Option<u64>,
    /// Reliably evidenced acceleration selected by the recommendation.
    pub acceleration: Option<AccelerationKind>,
    /// Whether approval adds a model to an externally managed runtime.
    pub external_runtime_effect: bool,
    /// Bounded reasons and warnings retained for restart-safe review.
    pub notices: Vec<PersistedSetupNotice>,
    /// Ordered exact material effect codes shown before approval.
    pub effect_codes: Vec<String>,
    /// UTC Unix timestamp in milliseconds.
    pub created_at_unix_ms: i64,
}

/// Authoritative persisted setup job snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedSetupJob {
    /// Immutable setup plan.
    pub plan: PersistedSetupPlan,
    /// Fine-grained authoritative job state.
    pub state: PersistedSetupState,
    /// Coarse user-visible stage.
    pub stage: PersistedSetupStage,
    /// Latest durable progress counters.
    pub progress: PersistedSetupProgress,
    /// Whether explicit cancellation has been requested.
    pub cancellation_requested: bool,
    /// Number of explicit retry requests.
    pub retry_count: u32,
    /// Stable safe error code without raw provider text.
    pub last_safe_error_code: Option<String>,
    /// Stable attention category without raw provider text.
    pub attention_reason: Option<String>,
    /// Runtime-health verification timestamp.
    pub runtime_verified_at_unix_ms: Option<i64>,
    /// Exact provider model-availability timestamp.
    pub model_available_at_unix_ms: Option<i64>,
    /// Provider-registration verification timestamp.
    pub registration_verified_at_unix_ms: Option<i64>,
    /// Fixed bounded-inference verification timestamp.
    pub inference_verified_at_unix_ms: Option<i64>,
    /// Durable approval timestamp.
    pub approved_at_unix_ms: Option<i64>,
    /// Durable cancellation completion timestamp.
    pub cancelled_at_unix_ms: Option<i64>,
    /// Latest durable event sequence.
    pub last_event_sequence: u64,
    /// Optimistic concurrency revision.
    pub revision: u64,
    /// Last update timestamp.
    pub updated_at_unix_ms: i64,
}

/// Exact approval input; plan scope is copied from SQLite rather than the caller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedSetupApprovalInput {
    /// Job being approved.
    pub job_id: JobId,
    /// Immutable plan revision displayed to the user.
    pub plan_revision: u32,
    /// Explicit user decision.
    pub decision: PersistedSetupApprovalDecision,
    /// Correlation identifier of the approval request.
    pub correlation_id: CorrelationId,
    /// Unique request identifier of the approval request.
    pub request_id: RequestId,
    /// UTC Unix decision timestamp in milliseconds.
    pub decided_at_unix_ms: i64,
}

/// Durable approval or denial reconstructed from the exact persisted plan snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedSetupApproval {
    /// Job whose plan received a decision.
    pub job_id: JobId,
    /// Immutable plan revision that received the decision.
    pub plan_revision: u32,
    /// Explicit durable decision.
    pub decision: PersistedSetupApprovalDecision,
    /// Canonical model identity copied from the plan.
    pub canonical_model_id: CandidateModelId,
    /// Runtime provider copied from the plan.
    pub provider_id: RuntimeProviderId,
    /// Exact provider artifact copied from the plan.
    pub provider_model_id: RuntimeProviderModelId,
    /// Approved destination category copied from the plan.
    pub destination_category: PersistedDestinationCategory,
    /// Conservative expected artifact size copied from the plan.
    pub expected_size_bytes: u64,
    /// SPDX licence copied from the immutable plan.
    pub licence_spdx: String,
    /// HTTPS provenance copied from the immutable plan.
    pub provenance_url: String,
    /// Whether the decision concerns an externally managed runtime.
    pub external_runtime_effect: bool,
    /// Ordered effects authorized by an approval; empty after denial.
    pub approved_effect_codes: Vec<String>,
    /// Correlation identifier of the decision request.
    pub correlation_id: CorrelationId,
    /// Unique identifier of the decision request.
    pub request_id: RequestId,
    /// UTC Unix decision timestamp in milliseconds.
    pub decided_at_unix_ms: i64,
}

/// One durable setup event returned through bounded replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedSetupEvent {
    /// Owning setup job.
    pub job_id: JobId,
    /// Monotonic per-job sequence.
    pub sequence: u64,
    /// Correlation identifier of the request or attempt that caused the event.
    pub correlation_id: CorrelationId,
    /// Event category.
    pub kind: PersistedSetupEventKind,
    /// Authoritative state after the event.
    pub state: PersistedSetupState,
    /// Authoritative stage after the event.
    pub stage: PersistedSetupStage,
    /// Latest durable progress.
    pub progress: PersistedSetupProgress,
    /// Stable safe event code.
    pub safe_code: Option<String>,
    /// UTC Unix event timestamp in milliseconds.
    pub occurred_at_unix_ms: i64,
}

/// One effect appended during a setup transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedSetupEffectInput {
    /// Effect disposition.
    pub disposition: PersistedEffectDisposition,
    /// Stable effect code.
    pub effect_code: String,
    /// Bounded effect size when meaningful.
    pub size_bytes: Option<u64>,
}

/// One persisted setup effect with deterministic ordering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedSetupEffect {
    /// Stable ordinal within the job.
    pub ordinal: u64,
    /// Effect disposition.
    pub disposition: PersistedEffectDisposition,
    /// Stable effect code.
    pub effect_code: String,
    /// Bounded effect size when meaningful.
    pub size_bytes: Option<u64>,
    /// UTC Unix timestamp in milliseconds.
    pub occurred_at_unix_ms: i64,
}

/// Full replacement state for the selected provider artifact in one transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedArtifactUpdate {
    /// New lifecycle state.
    pub lifecycle: PersistedModelLifecycle,
    /// New aggregate verification state.
    pub verification: PersistedModelVerification,
    /// New integrity evidence state.
    pub integrity: PersistedModelIntegrity,
    /// Provider-reported measured size when validated.
    pub measured_size_bytes: Option<u64>,
    /// Provider-registration timestamp when verified.
    pub registered_at_unix_ms: Option<i64>,
    /// Full readiness verification timestamp when verified.
    pub verified_at_unix_ms: Option<i64>,
}

/// Persisted provider artifact metadata separate from canonical model identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedModelArtifact {
    /// Canonical model identity.
    pub canonical_model_id: CandidateModelId,
    /// Runtime provider identity.
    pub provider_id: RuntimeProviderId,
    /// Opaque provider artifact identity.
    pub provider_model_id: RuntimeProviderModelId,
    /// Bounded provider-neutral artifact source summary.
    pub artifact_source_summary: String,
    /// Bounded destination label containing no private local path.
    pub destination_display: String,
    /// Privacy-preserving destination category.
    pub destination_category: PersistedDestinationCategory,
    /// Expected artifact size.
    pub expected_size_bytes: u64,
    /// Provider-reported measured size.
    pub measured_size_bytes: Option<u64>,
    /// Artifact lifecycle state.
    pub lifecycle: PersistedModelLifecycle,
    /// Aggregate verification state.
    pub verification: PersistedModelVerification,
    /// Integrity evidence state.
    pub integrity: PersistedModelIntegrity,
    /// Most recent setup job associated with this artifact.
    pub last_setup_job_id: Option<JobId>,
    /// Provider-registration timestamp.
    pub registered_at_unix_ms: Option<i64>,
    /// Full readiness verification timestamp.
    pub verified_at_unix_ms: Option<i64>,
    /// Last update timestamp.
    pub updated_at_unix_ms: i64,
}

/// Atomic state transition requested by the core-owned setup state machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedSetupTransition {
    /// Job to update.
    pub job_id: JobId,
    /// State the core expects before the transition.
    pub expected_state: PersistedSetupState,
    /// Revision the core expects before the transition.
    pub expected_revision: u64,
    /// New authoritative state.
    pub next_state: PersistedSetupState,
    /// New authoritative stage.
    pub next_stage: PersistedSetupStage,
    /// New progress, or no change when absent.
    pub progress: Option<PersistedSetupProgress>,
    /// Whether prior progress must be cleared.
    pub clear_progress: bool,
    /// Stable safe error code, replacing any prior code.
    pub safe_error_code: Option<String>,
    /// Stable attention reason, replacing any prior reason.
    pub attention_reason: Option<String>,
    /// Newly verified runtime-health timestamp.
    pub runtime_verified_at_unix_ms: Option<i64>,
    /// Newly verified model-availability timestamp.
    pub model_available_at_unix_ms: Option<i64>,
    /// Newly verified provider-registration timestamp.
    pub registration_verified_at_unix_ms: Option<i64>,
    /// Newly verified readiness-inference timestamp.
    pub inference_verified_at_unix_ms: Option<i64>,
    /// Whether all previous verification timestamps must be cleared.
    pub clear_verification: bool,
    /// Whether durable cancellation intent must be cleared for retry.
    pub clear_cancellation: bool,
    /// Whether the retry counter must increment.
    pub increment_retry: bool,
    /// Optional provider-artifact replacement committed atomically.
    pub artifact_update: Option<PersistedArtifactUpdate>,
    /// Bounded categorical effects appended atomically.
    pub effects: Vec<PersistedSetupEffectInput>,
    /// Durable event category emitted after the transition.
    pub event_kind: PersistedSetupEventKind,
    /// Correlation identifier of the request or attempt causing the transition.
    pub correlation_id: CorrelationId,
    /// UTC Unix transition timestamp in milliseconds.
    pub occurred_at_unix_ms: i64,
}

/// Revision and event sequence returned after one successful write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistedSetupWriteResult {
    /// New optimistic revision.
    pub revision: u64,
    /// New durable event sequence.
    pub event_sequence: u64,
}

/// Typed repository for setup jobs, approvals, progress, effects, and model metadata.
#[derive(Clone)]
pub struct SetupJobRepository {
    persistence: Persistence,
}

struct CancellationStateRow {
    state: String,
    stage: String,
    revision: i64,
    sequence: i64,
    phase: Option<String>,
    completed: Option<i64>,
    total: Option<i64>,
    basis_points: Option<i64>,
}

impl SetupJobRepository {
    pub(crate) fn new(persistence: Persistence) -> Self {
        Self { persistence }
    }

    /// Creates one immutable setup plan and its first durable event atomically.
    pub fn create(
        &self,
        plan: &PersistedSetupPlan,
        correlation_id: CorrelationId,
    ) -> Result<PersistedSetupJob, PersistenceError> {
        validate_plan(plan)?;
        self.persistence.with_write_transaction(|transaction| {
            let active_exists = transaction
                .query_row(
                    "SELECT EXISTS(
                         SELECT 1 FROM setup_jobs
                         WHERE state NOT IN ('ready', 'failed', 'cancelled')
                     )",
                    [],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(|source| PersistenceError::sqlite("check_active_setup_job", source))?;
            if active_exists {
                return Err(PersistenceError::RecordConflict {
                    entity: "setup_job",
                });
            }

            transaction
                .execute(
                    "INSERT INTO models
                     (canonical_model_id, display_name, family, size_class, licence_spdx,
                      provenance_url, catalogue_version, expected_size_bytes,
                      created_at_unix_ms, updated_at_unix_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
                     ON CONFLICT(canonical_model_id) DO UPDATE SET
                         display_name = excluded.display_name,
                         family = excluded.family,
                         size_class = excluded.size_class,
                         licence_spdx = excluded.licence_spdx,
                         provenance_url = excluded.provenance_url,
                         catalogue_version = excluded.catalogue_version,
                         expected_size_bytes = excluded.expected_size_bytes,
                         updated_at_unix_ms = excluded.updated_at_unix_ms",
                    params![
                        plan.canonical_model_id.as_str(),
                        plan.model_display_name,
                        plan.model_family,
                        model_size_class_as_str(plan.model_size_class)?,
                        plan.licence_spdx,
                        plan.provenance_url,
                        plan.catalogue_version.as_str(),
                        to_i64(plan.expected_size_bytes, "expected_size_bytes")?,
                        plan.created_at_unix_ms,
                    ],
                )
                .map_err(|source| PersistenceError::sqlite("write_setup_model", source))?;

            let existing_provider_model_id = transaction
                .query_row(
                    "SELECT provider_model_id FROM model_provider_artifacts
                     WHERE canonical_model_id = ?1 AND provider_id = ?2",
                    params![plan.canonical_model_id.as_str(), plan.provider_id.as_str()],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(|source| PersistenceError::sqlite("read_setup_artifact", source))?;
            if existing_provider_model_id
                .as_deref()
                .is_some_and(|value| value != plan.provider_model_id.as_str())
            {
                return Err(PersistenceError::RecordConflict {
                    entity: "model_provider_artifact",
                });
            }

            transaction
                .execute(
                    "INSERT INTO durable_jobs (id, kind, state, updated_at_unix_ms)
                     VALUES (?1, 'model_setup', 'queued', ?2)",
                    params![plan.job_id.as_str(), plan.created_at_unix_ms],
                )
                .map_err(|source| PersistenceError::sqlite("write_setup_job_envelope", source))?;

            transaction
                .execute(
                    "INSERT INTO setup_jobs
                     (job_id, plan_schema_version, plan_revision, state, stage, canonical_model_id,
                      model_display_name, model_family, model_size_class, licence_spdx,
                      provenance_url, provider_id, provider_model_id,
                      artifact_source_summary, destination_display, runtime_display_name,
                      runtime_version, catalogue_version, rule_set_version,
                      destination_category, expected_size_bytes, expected_memory_bytes,
                      memory_safety_margin_bytes, storage_safety_margin_bytes,
                      planned_context_tokens, cpu_only, gpu_memory_bytes,
                      observed_total_memory_bytes, observed_available_memory_bytes,
                      observed_free_storage_bytes, acceleration,
                      external_runtime_effect, last_event_sequence, revision,
                     created_at_unix_ms, updated_at_unix_ms)
                     VALUES
                     (?1, ?2, ?3, 'awaiting_approval', 'approval', ?4, ?5, ?6, ?7, ?8,
                      ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19,
                      ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29,
                      ?30, 1, 0, ?31, ?31)",
                    params![
                        plan.job_id.as_str(),
                        plan.plan_schema_version,
                        plan.plan_revision,
                        plan.canonical_model_id.as_str(),
                        plan.model_display_name,
                        plan.model_family,
                        model_size_class_as_str(plan.model_size_class)?,
                        plan.licence_spdx,
                        plan.provenance_url,
                        plan.provider_id.as_str(),
                        plan.provider_model_id.as_str(),
                        plan.artifact_source_summary,
                        plan.destination_display,
                        plan.runtime_display_name,
                        plan.runtime_version,
                        plan.catalogue_version.as_str(),
                        plan.rule_set_version.as_str(),
                        plan.destination_category.as_str(),
                        to_i64(plan.expected_size_bytes, "expected_size_bytes")?,
                        to_i64(plan.expected_memory_bytes, "expected_memory_bytes")?,
                        to_i64(
                            plan.memory_safety_margin_bytes,
                            "memory_safety_margin_bytes"
                        )?,
                        to_i64(
                            plan.storage_safety_margin_bytes,
                            "storage_safety_margin_bytes"
                        )?,
                        i64::from(plan.planned_context_tokens),
                        plan.cpu_only,
                        plan.gpu_memory_bytes
                            .map(|value| to_i64(value, "gpu_memory_bytes"))
                            .transpose()?,
                        plan.observed_total_memory_bytes
                            .map(|value| to_i64(value, "observed_total_memory_bytes"))
                            .transpose()?,
                        plan.observed_available_memory_bytes
                            .map(|value| to_i64(value, "observed_available_memory_bytes"))
                            .transpose()?,
                        plan.observed_free_storage_bytes
                            .map(|value| to_i64(value, "observed_free_storage_bytes"))
                            .transpose()?,
                        plan.acceleration.map(acceleration_as_str).transpose()?,
                        plan.external_runtime_effect,
                        plan.created_at_unix_ms,
                    ],
                )
                .map_err(|source| PersistenceError::sqlite("write_setup_job", source))?;

            for (ordinal, notice) in plan.notices.iter().enumerate() {
                let ordinal =
                    i64::try_from(ordinal).map_err(|_| PersistenceError::InvalidRecord {
                        field: "setup_notice_ordinal",
                    })?;
                transaction
                    .execute(
                        "INSERT INTO setup_plan_notices (job_id, ordinal, kind, code, message)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![
                            plan.job_id.as_str(),
                            ordinal,
                            if notice.warning { "warning" } else { "reason" },
                            notice.code,
                            notice.message,
                        ],
                    )
                    .map_err(|source| {
                        PersistenceError::sqlite("write_setup_plan_notice", source)
                    })?;
            }
            for (ordinal, effect_code) in plan.effect_codes.iter().enumerate() {
                let ordinal =
                    i64::try_from(ordinal).map_err(|_| PersistenceError::InvalidRecord {
                        field: "setup_effect_ordinal",
                    })?;
                transaction
                    .execute(
                        "INSERT INTO setup_plan_effects (job_id, ordinal, effect_code)
                         VALUES (?1, ?2, ?3)",
                        params![plan.job_id.as_str(), ordinal, effect_code],
                    )
                    .map_err(|source| {
                        PersistenceError::sqlite("write_setup_plan_effect", source)
                    })?;
            }

            transaction
                .execute(
                    "INSERT INTO model_provider_artifacts
                     (canonical_model_id, provider_id, provider_model_id,
                      artifact_source_summary, destination_display, destination_category,
                      expected_size_bytes, lifecycle_state, verification_state, integrity_state,
                      last_setup_job_id, updated_at_unix_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'planned', 'not_started',
                             'unavailable', ?8, ?9)
                     ON CONFLICT(canonical_model_id, provider_id) DO UPDATE SET
                         artifact_source_summary = excluded.artifact_source_summary,
                         destination_display = excluded.destination_display,
                         last_setup_job_id = excluded.last_setup_job_id,
                         updated_at_unix_ms = excluded.updated_at_unix_ms",
                    params![
                        plan.canonical_model_id.as_str(),
                        plan.provider_id.as_str(),
                        plan.provider_model_id.as_str(),
                        plan.artifact_source_summary,
                        plan.destination_display,
                        plan.destination_category.as_str(),
                        to_i64(plan.expected_size_bytes, "expected_size_bytes")?,
                        plan.job_id.as_str(),
                        plan.created_at_unix_ms,
                    ],
                )
                .map_err(|source| PersistenceError::sqlite("write_setup_artifact", source))?;

            insert_event(
                transaction,
                &plan.job_id,
                1,
                correlation_id,
                PersistedSetupEventKind::Snapshot,
                PersistedSetupState::AwaitingApproval,
                PersistedSetupStage::Approval,
                PersistedSetupProgress::default(),
                None,
                plan.created_at_unix_ms,
            )?;
            prune_events(transaction, &plan.job_id)
        })?;
        self.get(&plan.job_id)?
            .ok_or(PersistenceError::RecordNotFound {
                entity: "setup_job",
            })
    }

    /// Reads one complete setup job snapshot including immutable notices/effects.
    pub fn get(&self, job_id: &JobId) -> Result<Option<PersistedSetupJob>, PersistenceError> {
        self.persistence.with_connection(|connection| {
            let mut statement = connection
                .prepare(SETUP_JOB_SELECT)
                .map_err(|source| PersistenceError::sqlite("prepare_setup_job_read", source))?;
            let row = statement
                .query_row(params![job_id.as_str()], decode_job_row)
                .optional()
                .map_err(|source| PersistenceError::sqlite("read_setup_job", source))?;
            let Some(mut job) = row.map(decode_job_values).transpose()? else {
                return Ok(None);
            };
            job.plan.notices = read_notices(connection, job_id)?;
            job.plan.effect_codes = read_plan_effects(connection, job_id)?;
            Ok(Some(job))
        })
    }

    /// Returns every interrupted active job in deterministic oldest-first order.
    pub fn interrupted_jobs(&self) -> Result<Vec<PersistedSetupJob>, PersistenceError> {
        let identifiers = self.persistence.with_connection(|connection| {
            let mut statement = connection
                .prepare(
                    "SELECT job_id FROM setup_jobs
                     WHERE state IN (
                         'preparing', 'checking_storage', 'acquiring', 'registering',
                         'verifying_runtime', 'verifying_model', 'running_test_inference'
                     )
                     ORDER BY updated_at_unix_ms, job_id",
                )
                .map_err(|source| {
                    PersistenceError::sqlite("prepare_interrupted_setup_jobs", source)
                })?;
            statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|source| PersistenceError::sqlite("read_interrupted_setup_jobs", source))?
                .map(|row| {
                    let value = row.map_err(|source| {
                        PersistenceError::sqlite("read_interrupted_setup_job", source)
                    })?;
                    JobId::new(value)
                })
                .collect::<Result<Vec<_>, _>>()
        })?;
        identifiers
            .iter()
            .map(|id| {
                self.get(id)?.ok_or(PersistenceError::RecordNotFound {
                    entity: "setup_job",
                })
            })
            .collect()
    }

    /// Returns the newest durable job, including terminal outcomes after restart.
    pub fn latest_recoverable(&self) -> Result<Option<PersistedSetupJob>, PersistenceError> {
        let identifier = self.persistence.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT job_id FROM setup_jobs
                     ORDER BY updated_at_unix_ms DESC, job_id DESC LIMIT 1",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(|source| PersistenceError::sqlite("read_recoverable_setup_job", source))
        })?;
        identifier
            .map(JobId::new)
            .transpose()?
            .map(|id| self.get(&id))
            .transpose()
            .map(Option::flatten)
    }

    /// Atomically records an immutable approval or denial and advances the job.
    pub fn decide_approval(
        &self,
        approval: &PersistedSetupApprovalInput,
    ) -> Result<PersistedSetupWriteResult, PersistenceError> {
        validate_timestamp(approval.decided_at_unix_ms)?;
        if approval.plan_revision == 0 {
            return Err(PersistenceError::InvalidRecord {
                field: "plan_revision",
            });
        }
        let plan_revision = i64::from(approval.plan_revision);
        self.persistence.try_with_write_transaction(|connection| {
            let (state, workflow_revision, last_sequence, cancellation_requested): (
                String,
                i64,
                i64,
                bool,
            ) = connection
                .query_row(
                    "SELECT state, revision, last_event_sequence, cancellation_requested
                     FROM setup_jobs
                     WHERE job_id = ?1 AND plan_revision = ?2",
                    params![approval.job_id.as_str(), plan_revision],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()
                .map_err(|source| PersistenceError::sqlite("read_setup_for_approval", source))?
                .ok_or(PersistenceError::RecordConflict {
                    entity: "setup_job",
                })?;
            if PersistedSetupState::parse(&state)? != PersistedSetupState::AwaitingApproval
                || cancellation_requested
            {
                return Err(PersistenceError::RecordConflict {
                    entity: "setup_job",
                });
            }
            let sequence = checked_increment(last_sequence, "setup_event_sequence")?;
            connection
                .execute(
                    "INSERT INTO setup_approvals
                     (job_id, plan_revision, decision, canonical_model_id, provider_id,
                      provider_model_id, destination_category, expected_size_bytes,
                      licence_spdx, provenance_url, external_runtime_effect,
                      correlation_id, request_id, decided_at_unix_ms)
                     SELECT setup_jobs.job_id, ?2, ?3, setup_jobs.canonical_model_id,
                            setup_jobs.provider_id, setup_jobs.provider_model_id,
                            setup_jobs.destination_category, setup_jobs.expected_size_bytes,
                            setup_jobs.licence_spdx, setup_jobs.provenance_url,
                            setup_jobs.external_runtime_effect, ?4, ?5, ?6
                     FROM setup_jobs
                     WHERE setup_jobs.job_id = ?1",
                    params![
                        approval.job_id.as_str(),
                        plan_revision,
                        approval.decision.as_str(),
                        approval.correlation_id.to_string(),
                        approval.request_id.to_string(),
                        approval.decided_at_unix_ms,
                    ],
                )
                .map_err(|source| PersistenceError::sqlite("write_setup_approval", source))?;
            if approval.decision == PersistedSetupApprovalDecision::Approve {
                connection
                    .execute(
                        "INSERT INTO setup_approval_effects (job_id, ordinal, effect_code)
                         SELECT job_id, ordinal, effect_code FROM setup_plan_effects
                         WHERE job_id = ?1 ORDER BY ordinal",
                        params![approval.job_id.as_str()],
                    )
                    .map_err(|source| {
                        PersistenceError::sqlite("write_setup_approval_effects", source)
                    })?;
            }
            let (next_state, next_stage, approved_at, cancelled_at, job_state, event_kind) =
                match approval.decision {
                    PersistedSetupApprovalDecision::Approve => (
                        PersistedSetupState::Approved,
                        PersistedSetupStage::Preparation,
                        Some(approval.decided_at_unix_ms),
                        None,
                        "queued",
                        PersistedSetupEventKind::StageChanged,
                    ),
                    PersistedSetupApprovalDecision::Deny => (
                        PersistedSetupState::Cancelled,
                        PersistedSetupStage::Cancelled,
                        None,
                        Some(approval.decided_at_unix_ms),
                        "cancelled",
                        PersistedSetupEventKind::Terminal,
                    ),
                };
            let updated = connection
                .execute(
                    "UPDATE setup_jobs SET
                         state = ?3, stage = ?4, approved_at_unix_ms = ?5,
                         cancelled_at_unix_ms = ?6, last_event_sequence = ?7,
                         revision = revision + 1,
                         updated_at_unix_ms = ?8
                     WHERE job_id = ?1 AND revision = ?2 AND plan_revision = ?9
                       AND state = 'awaiting_approval' AND cancellation_requested = 0",
                    params![
                        approval.job_id.as_str(),
                        workflow_revision,
                        next_state.as_str(),
                        next_stage.as_str(),
                        approved_at,
                        cancelled_at,
                        sequence,
                        approval.decided_at_unix_ms,
                        plan_revision,
                    ],
                )
                .map_err(|source| PersistenceError::sqlite("advance_setup_approval", source))?;
            if updated != 1 {
                return Err(PersistenceError::RecordConflict {
                    entity: "setup_job",
                });
            }
            connection
                .execute(
                    "UPDATE durable_jobs SET state = ?2, updated_at_unix_ms = ?3
                     WHERE id = ?1",
                    params![
                        approval.job_id.as_str(),
                        job_state,
                        approval.decided_at_unix_ms
                    ],
                )
                .map_err(|source| PersistenceError::sqlite("update_setup_job_envelope", source))?;
            insert_event_on_connection(
                connection,
                &approval.job_id,
                sequence,
                approval.correlation_id,
                event_kind,
                next_state,
                next_stage,
                PersistedSetupProgress::default(),
                None,
                approval.decided_at_unix_ms,
            )?;
            prune_events(connection, &approval.job_id)?;
            Ok(PersistedSetupWriteResult {
                revision: to_u64(workflow_revision, "setup_revision")?.saturating_add(1),
                event_sequence: to_u64(sequence, "setup_event_sequence")?,
            })
        })
    }

    /// Reads the durable approval or denial and its exact authorized effects.
    pub fn approval(
        &self,
        job_id: &JobId,
    ) -> Result<Option<PersistedSetupApproval>, PersistenceError> {
        self.persistence.with_connection(|connection| {
            let raw = connection
                .query_row(
                    "SELECT plan_revision, decision, canonical_model_id, provider_id,
                            provider_model_id, destination_category, expected_size_bytes,
                            licence_spdx, provenance_url, external_runtime_effect,
                            correlation_id, request_id, decided_at_unix_ms
                     FROM setup_approvals WHERE job_id = ?1",
                    params![job_id.as_str()],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, String>(5)?,
                            row.get::<_, i64>(6)?,
                            row.get::<_, String>(7)?,
                            row.get::<_, String>(8)?,
                            row.get::<_, bool>(9)?,
                            row.get::<_, String>(10)?,
                            row.get::<_, String>(11)?,
                            row.get::<_, i64>(12)?,
                        ))
                    },
                )
                .optional()
                .map_err(|source| PersistenceError::sqlite("read_setup_approval", source))?;
            raw.map(
                |(
                    plan_revision,
                    decision,
                    canonical_model_id,
                    provider_id,
                    provider_model_id,
                    destination,
                    expected_size,
                    licence_spdx,
                    provenance_url,
                    external_runtime_effect,
                    correlation_id,
                    request_id,
                    decided_at_unix_ms,
                )| {
                    Ok(PersistedSetupApproval {
                        job_id: job_id.clone(),
                        plan_revision: to_u32(plan_revision, "plan_revision")?,
                        decision: PersistedSetupApprovalDecision::parse(&decision)?,
                        canonical_model_id: CandidateModelId::new(canonical_model_id),
                        provider_id: RuntimeProviderId::new(provider_id),
                        provider_model_id: RuntimeProviderModelId::new(provider_model_id),
                        destination_category: PersistedDestinationCategory::parse(&destination)?,
                        expected_size_bytes: to_u64(expected_size, "expected_size_bytes")?,
                        licence_spdx,
                        provenance_url,
                        external_runtime_effect,
                        approved_effect_codes: read_approval_effects(connection, job_id)?,
                        correlation_id: correlation_id.parse().map_err(|_| {
                            PersistenceError::InvalidRecord {
                                field: "approval_correlation_id",
                            }
                        })?,
                        request_id: request_id.parse().map_err(|_| {
                            PersistenceError::InvalidRecord {
                                field: "approval_request_id",
                            }
                        })?,
                        decided_at_unix_ms,
                    })
                },
            )
            .transpose()
        })
    }

    /// Persists cancellation intent before signalling any in-memory token.
    pub fn request_cancellation(
        &self,
        job_id: &JobId,
        correlation_id: CorrelationId,
        occurred_at_unix_ms: i64,
    ) -> Result<PersistedSetupWriteResult, PersistenceError> {
        validate_timestamp(occurred_at_unix_ms)?;
        self.persistence.try_with_write_transaction(|connection| {
            let row = connection
                .query_row(
                    "SELECT state, stage, revision, last_event_sequence,
                            progress_phase, progress_completed_bytes,
                            progress_total_bytes, progress_basis_points
                     FROM setup_jobs WHERE job_id = ?1",
                    params![job_id.as_str()],
                    |row| {
                        Ok(CancellationStateRow {
                            state: row.get(0)?,
                            stage: row.get(1)?,
                            revision: row.get(2)?,
                            sequence: row.get(3)?,
                            phase: row.get(4)?,
                            completed: row.get(5)?,
                            total: row.get(6)?,
                            basis_points: row.get(7)?,
                        })
                    },
                )
                .optional()
                .map_err(|source| PersistenceError::sqlite("read_setup_for_cancel", source))?
                .ok_or(PersistenceError::RecordNotFound {
                    entity: "setup_job",
                })?;
            let state = PersistedSetupState::parse(&row.state)?;
            if state.is_terminal() {
                return Err(PersistenceError::RecordConflict {
                    entity: "setup_job",
                });
            }
            let next_sequence = checked_increment(row.sequence, "setup_event_sequence")?;
            let updated = connection
                .execute(
                    "UPDATE setup_jobs SET cancellation_requested = 1,
                         last_event_sequence = ?3, revision = revision + 1,
                         updated_at_unix_ms = ?4
                     WHERE job_id = ?1 AND revision = ?2",
                    params![
                        job_id.as_str(),
                        row.revision,
                        next_sequence,
                        occurred_at_unix_ms
                    ],
                )
                .map_err(|source| PersistenceError::sqlite("request_setup_cancel", source))?;
            if updated != 1 {
                return Err(PersistenceError::RecordConflict {
                    entity: "setup_job",
                });
            }
            let progress = PersistedSetupProgress {
                phase: row.phase.as_deref().map(parse_progress_phase).transpose()?,
                completed_bytes: optional_u64(row.completed, "setup_progress_completed")?,
                total_bytes: optional_u64(row.total, "setup_progress_total")?,
                basis_points: optional_u16(row.basis_points, "setup_progress_basis_points")?,
            }
            .validate()?;
            insert_event_on_connection(
                connection,
                job_id,
                next_sequence,
                correlation_id,
                PersistedSetupEventKind::CancellationRequested,
                state,
                PersistedSetupStage::parse(&row.stage)?,
                progress,
                Some("setup.cancellation_requested"),
                occurred_at_unix_ms,
            )?;
            prune_events(connection, job_id)?;
            Ok(PersistedSetupWriteResult {
                revision: to_u64(row.revision, "setup_revision")?.saturating_add(1),
                event_sequence: to_u64(next_sequence, "setup_event_sequence")?,
            })
        })
    }

    /// Applies one core-validated state transition as a single SQLite commit.
    pub fn transition(
        &self,
        transition: &PersistedSetupTransition,
    ) -> Result<PersistedSetupWriteResult, PersistenceError> {
        validate_transition(transition)?;
        self.persistence.with_write_transaction(|transaction| {
            let current = read_transition_state(transaction, &transition.job_id)?;
            if current.state != transition.expected_state
                || current.revision != transition.expected_revision
            {
                return Err(PersistenceError::RecordConflict {
                    entity: "setup_job",
                });
            }

            let progress = next_progress(&current.progress, transition)?;
            let runtime_verified = next_evidence(
                current.runtime_verified_at_unix_ms,
                transition.runtime_verified_at_unix_ms,
                transition.clear_verification,
            );
            let model_available = next_evidence(
                current.model_available_at_unix_ms,
                transition.model_available_at_unix_ms,
                transition.clear_verification,
            );
            let registration_verified = next_evidence(
                current.registration_verified_at_unix_ms,
                transition.registration_verified_at_unix_ms,
                transition.clear_verification,
            );
            let inference_verified = next_evidence(
                current.inference_verified_at_unix_ms,
                transition.inference_verified_at_unix_ms,
                transition.clear_verification,
            );
            if transition.next_state == PersistedSetupState::Ready
                && (runtime_verified.is_none()
                    || model_available.is_none()
                    || registration_verified.is_none()
                    || inference_verified.is_none()
                    || !transition.artifact_update.as_ref().is_some_and(|update| {
                        update.lifecycle == PersistedModelLifecycle::Available
                            && update.verification == PersistedModelVerification::Verified
                            && matches!(
                                update.integrity,
                                PersistedModelIntegrity::ProviderReported
                                    | PersistedModelIntegrity::Verified
                            )
                            && update.registered_at_unix_ms.is_some()
                            && update.verified_at_unix_ms.is_some()
                    }))
            {
                return Err(PersistenceError::InvalidRecord {
                    field: "setup_ready_evidence",
                });
            }
            if transition
                .artifact_update
                .as_ref()
                .is_some_and(|update| update.lifecycle == PersistedModelLifecycle::Available)
                && transition.next_state != PersistedSetupState::Ready
            {
                return Err(PersistenceError::InvalidRecord {
                    field: "model_available_before_ready",
                });
            }

            let next_sequence = current.event_sequence.saturating_add(1);
            let next_revision = current.revision.saturating_add(1);
            let retry_count = current
                .retry_count
                .saturating_add(u32::from(transition.increment_retry));
            let cancellation_requested = if transition.clear_cancellation {
                false
            } else {
                current.cancellation_requested
            };
            let cancelled_at = if transition.next_state == PersistedSetupState::Cancelled {
                Some(transition.occurred_at_unix_ms)
            } else {
                current.cancelled_at_unix_ms
            };
            let updated = transaction
                .execute(
                    "UPDATE setup_jobs SET
                         state = ?3, stage = ?4,
                         progress_phase = ?5, progress_completed_bytes = ?6,
                         progress_total_bytes = ?7, progress_basis_points = ?8,
                         cancellation_requested = ?9, retry_count = ?10,
                         last_safe_error_code = ?11, attention_reason = ?12,
                         runtime_verified_at_unix_ms = ?13,
                         model_available_at_unix_ms = ?14,
                         registration_verified_at_unix_ms = ?15,
                         inference_verified_at_unix_ms = ?16,
                         cancelled_at_unix_ms = ?17, last_event_sequence = ?18,
                         revision = ?19, updated_at_unix_ms = ?20
                     WHERE job_id = ?1 AND revision = ?2",
                    params![
                        transition.job_id.as_str(),
                        to_i64(current.revision, "setup_revision")?,
                        transition.next_state.as_str(),
                        transition.next_stage.as_str(),
                        progress.phase.map(progress_phase_as_str).transpose()?,
                        progress
                            .completed_bytes
                            .map(|value| to_i64(value, "setup_progress_completed"))
                            .transpose()?,
                        progress
                            .total_bytes
                            .map(|value| to_i64(value, "setup_progress_total"))
                            .transpose()?,
                        progress.basis_points.map(i64::from),
                        cancellation_requested,
                        retry_count,
                        transition.safe_error_code,
                        transition.attention_reason,
                        runtime_verified,
                        model_available,
                        registration_verified,
                        inference_verified,
                        cancelled_at,
                        to_i64(next_sequence, "setup_event_sequence")?,
                        to_i64(next_revision, "setup_revision")?,
                        transition.occurred_at_unix_ms,
                    ],
                )
                .map_err(|source| PersistenceError::sqlite("transition_setup_job", source))?;
            if updated != 1 {
                return Err(PersistenceError::RecordConflict {
                    entity: "setup_job",
                });
            }

            transaction
                .execute(
                    "UPDATE durable_jobs SET state = ?2, updated_at_unix_ms = ?3 WHERE id = ?1",
                    params![
                        transition.job_id.as_str(),
                        durable_job_state(transition.next_state),
                        transition.occurred_at_unix_ms,
                    ],
                )
                .map_err(|source| PersistenceError::sqlite("transition_setup_envelope", source))?;

            if let Some(update) = &transition.artifact_update {
                validate_artifact_update(update)?;
                let changed = transaction
                    .execute(
                        "UPDATE model_provider_artifacts SET
                             measured_size_bytes = ?2, lifecycle_state = ?3,
                             verification_state = ?4, integrity_state = ?5,
                             last_setup_job_id = ?1, registered_at_unix_ms = ?6,
                             verified_at_unix_ms = ?7, updated_at_unix_ms = ?8
                         WHERE canonical_model_id = (
                             SELECT canonical_model_id FROM setup_jobs WHERE job_id = ?1
                         ) AND provider_id = (
                             SELECT provider_id FROM setup_jobs WHERE job_id = ?1
                         )",
                        params![
                            transition.job_id.as_str(),
                            update
                                .measured_size_bytes
                                .map(|value| to_i64(value, "measured_size_bytes"))
                                .transpose()?,
                            update.lifecycle.as_str(),
                            update.verification.as_str(),
                            update.integrity.as_str(),
                            update.registered_at_unix_ms,
                            update.verified_at_unix_ms,
                            transition.occurred_at_unix_ms,
                        ],
                    )
                    .map_err(|source| {
                        PersistenceError::sqlite("transition_setup_artifact", source)
                    })?;
                if changed != 1 {
                    return Err(PersistenceError::RecordNotFound {
                        entity: "model_provider_artifact",
                    });
                }
            }

            append_effects(transaction, transition, current.effect_count)?;
            let safe_code = transition
                .safe_error_code
                .as_deref()
                .or(transition.attention_reason.as_deref());
            insert_event(
                transaction,
                &transition.job_id,
                to_i64(next_sequence, "setup_event_sequence")?,
                transition.correlation_id,
                transition.event_kind,
                transition.next_state,
                transition.next_stage,
                progress,
                safe_code,
                transition.occurred_at_unix_ms,
            )?;
            prune_events(transaction, &transition.job_id)?;
            Ok(PersistedSetupWriteResult {
                revision: next_revision,
                event_sequence: next_sequence,
            })
        })
    }

    /// Lists a bounded page of durable events after an exclusive cursor.
    pub fn events_after(
        &self,
        job_id: &JobId,
        after_sequence: u64,
        limit: u16,
    ) -> Result<Vec<PersistedSetupEvent>, PersistenceError> {
        if limit == 0 || limit > EVENT_PAGE_MAX || after_sequence > i64::MAX as u64 {
            return Err(PersistenceError::InvalidRecord {
                field: "setup_event_page",
            });
        }
        self.persistence.with_connection(|connection| {
            let mut statement = connection
                .prepare(
                    "SELECT sequence, correlation_id, kind, state, stage, progress_phase,
                            progress_completed_bytes, progress_total_bytes,
                            progress_basis_points, safe_code,
                            occurred_at_unix_ms
                     FROM setup_job_events
                     WHERE job_id = ?1 AND sequence > ?2
                     ORDER BY sequence LIMIT ?3",
                )
                .map_err(|source| PersistenceError::sqlite("prepare_setup_event_page", source))?;
            statement
                .query_map(
                    params![job_id.as_str(), after_sequence as i64, limit],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, Option<String>>(5)?,
                            row.get::<_, Option<i64>>(6)?,
                            row.get::<_, Option<i64>>(7)?,
                            row.get::<_, Option<i64>>(8)?,
                            row.get::<_, Option<String>>(9)?,
                            row.get::<_, i64>(10)?,
                        ))
                    },
                )
                .map_err(|source| PersistenceError::sqlite("read_setup_event_page", source))?
                .map(|row| {
                    let (
                        sequence,
                        correlation_id,
                        kind,
                        state,
                        stage,
                        phase,
                        completed,
                        total,
                        basis,
                        code,
                        occurred,
                    ) =
                        row.map_err(|source| PersistenceError::sqlite("read_setup_event", source))?;
                    Ok(PersistedSetupEvent {
                        job_id: job_id.clone(),
                        sequence: to_u64(sequence, "setup_event_sequence")?,
                        correlation_id: correlation_id.parse().map_err(|_| {
                            PersistenceError::InvalidRecord {
                                field: "setup_event_correlation_id",
                            }
                        })?,
                        kind: PersistedSetupEventKind::parse(&kind)?,
                        state: PersistedSetupState::parse(&state)?,
                        stage: PersistedSetupStage::parse(&stage)?,
                        progress: PersistedSetupProgress {
                            phase: phase.as_deref().map(parse_progress_phase).transpose()?,
                            completed_bytes: optional_u64(completed, "setup_progress_completed")?,
                            total_bytes: optional_u64(total, "setup_progress_total")?,
                            basis_points: optional_u16(basis, "setup_progress_basis_points")?,
                        }
                        .validate()?,
                        safe_code: code,
                        occurred_at_unix_ms: occurred,
                    })
                })
                .collect()
        })
    }

    /// Lists every bounded effect for one job in deterministic order.
    pub fn effects(&self, job_id: &JobId) -> Result<Vec<PersistedSetupEffect>, PersistenceError> {
        self.persistence.with_connection(|connection| {
            let mut statement = connection
                .prepare(
                    "SELECT ordinal, disposition, effect_code, size_bytes, occurred_at_unix_ms
                     FROM setup_effects WHERE job_id = ?1 ORDER BY ordinal",
                )
                .map_err(|source| PersistenceError::sqlite("prepare_setup_effects", source))?;
            statement
                .query_map(params![job_id.as_str()], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                })
                .map_err(|source| PersistenceError::sqlite("read_setup_effects", source))?
                .map(|row| {
                    let (ordinal, disposition, effect_code, size_bytes, occurred) = row
                        .map_err(|source| PersistenceError::sqlite("read_setup_effect", source))?;
                    Ok(PersistedSetupEffect {
                        ordinal: to_u64(ordinal, "setup_effect_ordinal")?,
                        disposition: PersistedEffectDisposition::parse(&disposition)?,
                        effect_code,
                        size_bytes: optional_u64(size_bytes, "setup_effect_size")?,
                        occurred_at_unix_ms: occurred,
                    })
                })
                .collect()
        })
    }

    /// Reads provider artifact metadata for one canonical model/provider pair.
    pub fn artifact(
        &self,
        canonical_model_id: &CandidateModelId,
        provider_id: &RuntimeProviderId,
    ) -> Result<Option<PersistedModelArtifact>, PersistenceError> {
        validate_text(
            "canonical_model_id",
            canonical_model_id.as_str(),
            ID_BYTES_MAX,
        )?;
        validate_text("provider_id", provider_id.as_str(), ID_BYTES_MAX)?;
        self.persistence.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT provider_model_id, artifact_source_summary, destination_display,
                            destination_category, expected_size_bytes, measured_size_bytes,
                            lifecycle_state, verification_state, integrity_state,
                            last_setup_job_id, registered_at_unix_ms,
                            verified_at_unix_ms, updated_at_unix_ms
                     FROM model_provider_artifacts
                     WHERE canonical_model_id = ?1 AND provider_id = ?2",
                    params![canonical_model_id.as_str(), provider_id.as_str()],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, i64>(4)?,
                            row.get::<_, Option<i64>>(5)?,
                            row.get::<_, String>(6)?,
                            row.get::<_, String>(7)?,
                            row.get::<_, String>(8)?,
                            row.get::<_, Option<String>>(9)?,
                            row.get::<_, Option<i64>>(10)?,
                            row.get::<_, Option<i64>>(11)?,
                            row.get::<_, i64>(12)?,
                        ))
                    },
                )
                .optional()
                .map_err(|source| PersistenceError::sqlite("read_model_artifact", source))?
                .map(
                    |(
                        provider_model_id,
                        artifact_source_summary,
                        destination_display,
                        destination,
                        expected,
                        measured,
                        lifecycle,
                        verification,
                        integrity,
                        job_id,
                        registered,
                        verified,
                        updated,
                    )| {
                        Ok(PersistedModelArtifact {
                            canonical_model_id: canonical_model_id.clone(),
                            provider_id: provider_id.clone(),
                            provider_model_id: RuntimeProviderModelId::new(provider_model_id),
                            artifact_source_summary,
                            destination_display,
                            destination_category: PersistedDestinationCategory::parse(
                                &destination,
                            )?,
                            expected_size_bytes: to_u64(expected, "expected_size_bytes")?,
                            measured_size_bytes: optional_u64(measured, "measured_size_bytes")?,
                            lifecycle: PersistedModelLifecycle::parse(&lifecycle)?,
                            verification: PersistedModelVerification::parse(&verification)?,
                            integrity: PersistedModelIntegrity::parse(&integrity)?,
                            last_setup_job_id: job_id.map(JobId::new).transpose()?,
                            registered_at_unix_ms: registered,
                            verified_at_unix_ms: verified,
                            updated_at_unix_ms: updated,
                        })
                    },
                )
                .transpose()
        })
    }
}

const SETUP_JOB_SELECT: &str =
    "SELECT setup_jobs.job_id, setup_jobs.plan_schema_version, setup_jobs.plan_revision,
            setup_jobs.state, setup_jobs.stage, setup_jobs.canonical_model_id,
            setup_jobs.provider_id, setup_jobs.provider_model_id,
            setup_jobs.artifact_source_summary, setup_jobs.destination_display,
            setup_jobs.runtime_display_name, setup_jobs.runtime_version,
            setup_jobs.catalogue_version, setup_jobs.rule_set_version,
            setup_jobs.destination_category, setup_jobs.expected_size_bytes,
            setup_jobs.expected_memory_bytes, setup_jobs.memory_safety_margin_bytes,
            setup_jobs.storage_safety_margin_bytes, setup_jobs.planned_context_tokens,
            setup_jobs.cpu_only, setup_jobs.gpu_memory_bytes,
            setup_jobs.observed_total_memory_bytes,
            setup_jobs.observed_available_memory_bytes,
            setup_jobs.observed_free_storage_bytes, setup_jobs.acceleration,
            setup_jobs.external_runtime_effect, setup_jobs.progress_phase,
            setup_jobs.progress_completed_bytes, setup_jobs.progress_total_bytes,
            setup_jobs.progress_basis_points,
            setup_jobs.cancellation_requested, setup_jobs.retry_count,
            setup_jobs.last_safe_error_code, setup_jobs.attention_reason,
            setup_jobs.runtime_verified_at_unix_ms, setup_jobs.model_available_at_unix_ms,
            setup_jobs.registration_verified_at_unix_ms,
            setup_jobs.inference_verified_at_unix_ms, setup_jobs.approved_at_unix_ms,
            setup_jobs.cancelled_at_unix_ms, setup_jobs.last_event_sequence,
            setup_jobs.revision, setup_jobs.created_at_unix_ms,
            setup_jobs.updated_at_unix_ms, setup_jobs.model_display_name,
            setup_jobs.model_family, setup_jobs.model_size_class,
            setup_jobs.licence_spdx, setup_jobs.provenance_url,
            setup_jobs.catalogue_version, setup_jobs.expected_size_bytes
     FROM setup_jobs
     WHERE setup_jobs.job_id = ?1";

struct RawSetupJob {
    job_id: String,
    plan_schema_version: i64,
    plan_revision: i64,
    state: String,
    stage: String,
    canonical_model_id: String,
    provider_id: String,
    provider_model_id: String,
    artifact_source_summary: String,
    destination_display: String,
    runtime_display_name: String,
    runtime_version: Option<String>,
    catalogue_version: String,
    rule_set_version: String,
    destination_category: String,
    expected_size_bytes: i64,
    expected_memory_bytes: i64,
    memory_safety_margin_bytes: i64,
    storage_safety_margin_bytes: i64,
    planned_context_tokens: i64,
    cpu_only: bool,
    gpu_memory_bytes: Option<i64>,
    observed_total_memory_bytes: Option<i64>,
    observed_available_memory_bytes: Option<i64>,
    observed_free_storage_bytes: Option<i64>,
    acceleration: Option<String>,
    external_runtime_effect: bool,
    progress_phase: Option<String>,
    progress_completed_bytes: Option<i64>,
    progress_total_bytes: Option<i64>,
    progress_basis_points: Option<i64>,
    cancellation_requested: bool,
    retry_count: i64,
    last_safe_error_code: Option<String>,
    attention_reason: Option<String>,
    runtime_verified_at_unix_ms: Option<i64>,
    model_available_at_unix_ms: Option<i64>,
    registration_verified_at_unix_ms: Option<i64>,
    inference_verified_at_unix_ms: Option<i64>,
    approved_at_unix_ms: Option<i64>,
    cancelled_at_unix_ms: Option<i64>,
    last_event_sequence: i64,
    revision: i64,
    created_at_unix_ms: i64,
    updated_at_unix_ms: i64,
    model_display_name: String,
    model_family: String,
    model_size_class: String,
    licence_spdx: String,
    provenance_url: String,
    model_catalogue_version: String,
    model_expected_size_bytes: i64,
}

fn decode_job_row(row: &Row<'_>) -> rusqlite::Result<RawSetupJob> {
    Ok(RawSetupJob {
        job_id: row.get(0)?,
        plan_schema_version: row.get(1)?,
        plan_revision: row.get(2)?,
        state: row.get(3)?,
        stage: row.get(4)?,
        canonical_model_id: row.get(5)?,
        provider_id: row.get(6)?,
        provider_model_id: row.get(7)?,
        artifact_source_summary: row.get(8)?,
        destination_display: row.get(9)?,
        runtime_display_name: row.get(10)?,
        runtime_version: row.get(11)?,
        catalogue_version: row.get(12)?,
        rule_set_version: row.get(13)?,
        destination_category: row.get(14)?,
        expected_size_bytes: row.get(15)?,
        expected_memory_bytes: row.get(16)?,
        memory_safety_margin_bytes: row.get(17)?,
        storage_safety_margin_bytes: row.get(18)?,
        planned_context_tokens: row.get(19)?,
        cpu_only: row.get(20)?,
        gpu_memory_bytes: row.get(21)?,
        observed_total_memory_bytes: row.get(22)?,
        observed_available_memory_bytes: row.get(23)?,
        observed_free_storage_bytes: row.get(24)?,
        acceleration: row.get(25)?,
        external_runtime_effect: row.get(26)?,
        progress_phase: row.get(27)?,
        progress_completed_bytes: row.get(28)?,
        progress_total_bytes: row.get(29)?,
        progress_basis_points: row.get(30)?,
        cancellation_requested: row.get(31)?,
        retry_count: row.get(32)?,
        last_safe_error_code: row.get(33)?,
        attention_reason: row.get(34)?,
        runtime_verified_at_unix_ms: row.get(35)?,
        model_available_at_unix_ms: row.get(36)?,
        registration_verified_at_unix_ms: row.get(37)?,
        inference_verified_at_unix_ms: row.get(38)?,
        approved_at_unix_ms: row.get(39)?,
        cancelled_at_unix_ms: row.get(40)?,
        last_event_sequence: row.get(41)?,
        revision: row.get(42)?,
        created_at_unix_ms: row.get(43)?,
        updated_at_unix_ms: row.get(44)?,
        model_display_name: row.get(45)?,
        model_family: row.get(46)?,
        model_size_class: row.get(47)?,
        licence_spdx: row.get(48)?,
        provenance_url: row.get(49)?,
        model_catalogue_version: row.get(50)?,
        model_expected_size_bytes: row.get(51)?,
    })
}

fn decode_job_values(raw: RawSetupJob) -> Result<PersistedSetupJob, PersistenceError> {
    if raw.catalogue_version != raw.model_catalogue_version
        || raw.expected_size_bytes != raw.model_expected_size_bytes
    {
        return Err(PersistenceError::InvalidRecord {
            field: "setup_model_snapshot",
        });
    }
    let progress = PersistedSetupProgress {
        phase: raw
            .progress_phase
            .as_deref()
            .map(parse_progress_phase)
            .transpose()?,
        completed_bytes: optional_u64(raw.progress_completed_bytes, "setup_progress_completed")?,
        total_bytes: optional_u64(raw.progress_total_bytes, "setup_progress_total")?,
        basis_points: optional_u16(raw.progress_basis_points, "setup_progress_basis_points")?,
    }
    .validate()?;
    Ok(PersistedSetupJob {
        plan: PersistedSetupPlan {
            job_id: JobId::new(raw.job_id)?,
            plan_schema_version: to_u32(raw.plan_schema_version, "plan_schema_version")?,
            plan_revision: to_u32(raw.plan_revision, "plan_revision")?,
            canonical_model_id: CandidateModelId::new(raw.canonical_model_id),
            model_display_name: raw.model_display_name,
            model_family: raw.model_family,
            model_size_class: parse_model_size_class(&raw.model_size_class)?,
            licence_spdx: raw.licence_spdx,
            provenance_url: raw.provenance_url,
            provider_id: RuntimeProviderId::new(raw.provider_id),
            provider_model_id: RuntimeProviderModelId::new(raw.provider_model_id),
            artifact_source_summary: raw.artifact_source_summary,
            destination_display: raw.destination_display,
            runtime_display_name: raw.runtime_display_name,
            runtime_version: raw.runtime_version,
            catalogue_version: CatalogueVersion::new(raw.catalogue_version),
            rule_set_version: RuleSetVersion::new(raw.rule_set_version),
            destination_category: PersistedDestinationCategory::parse(&raw.destination_category)?,
            expected_size_bytes: to_u64(raw.expected_size_bytes, "expected_size_bytes")?,
            expected_memory_bytes: to_u64(raw.expected_memory_bytes, "expected_memory_bytes")?,
            memory_safety_margin_bytes: to_u64(
                raw.memory_safety_margin_bytes,
                "memory_safety_margin_bytes",
            )?,
            storage_safety_margin_bytes: to_u64(
                raw.storage_safety_margin_bytes,
                "storage_safety_margin_bytes",
            )?,
            planned_context_tokens: to_u32(raw.planned_context_tokens, "planned_context_tokens")?,
            cpu_only: raw.cpu_only,
            gpu_memory_bytes: optional_u64(raw.gpu_memory_bytes, "gpu_memory_bytes")?,
            observed_total_memory_bytes: optional_u64(
                raw.observed_total_memory_bytes,
                "observed_total_memory_bytes",
            )?,
            observed_available_memory_bytes: optional_u64(
                raw.observed_available_memory_bytes,
                "observed_available_memory_bytes",
            )?,
            observed_free_storage_bytes: optional_u64(
                raw.observed_free_storage_bytes,
                "observed_free_storage_bytes",
            )?,
            acceleration: raw
                .acceleration
                .as_deref()
                .map(parse_acceleration)
                .transpose()?,
            external_runtime_effect: raw.external_runtime_effect,
            notices: Vec::new(),
            effect_codes: Vec::new(),
            created_at_unix_ms: raw.created_at_unix_ms,
        },
        state: PersistedSetupState::parse(&raw.state)?,
        stage: PersistedSetupStage::parse(&raw.stage)?,
        progress,
        cancellation_requested: raw.cancellation_requested,
        retry_count: to_u32(raw.retry_count, "setup_retry_count")?,
        last_safe_error_code: raw.last_safe_error_code,
        attention_reason: raw.attention_reason,
        runtime_verified_at_unix_ms: raw.runtime_verified_at_unix_ms,
        model_available_at_unix_ms: raw.model_available_at_unix_ms,
        registration_verified_at_unix_ms: raw.registration_verified_at_unix_ms,
        inference_verified_at_unix_ms: raw.inference_verified_at_unix_ms,
        approved_at_unix_ms: raw.approved_at_unix_ms,
        cancelled_at_unix_ms: raw.cancelled_at_unix_ms,
        last_event_sequence: to_u64(raw.last_event_sequence, "setup_event_sequence")?,
        revision: to_u64(raw.revision, "setup_revision")?,
        updated_at_unix_ms: raw.updated_at_unix_ms,
    })
}

fn read_notices(
    connection: &rusqlite::Connection,
    job_id: &JobId,
) -> Result<Vec<PersistedSetupNotice>, PersistenceError> {
    let mut statement = connection
        .prepare(
            "SELECT kind, code, message FROM setup_plan_notices
             WHERE job_id = ?1 ORDER BY ordinal",
        )
        .map_err(|source| PersistenceError::sqlite("prepare_setup_notices", source))?;
    statement
        .query_map(params![job_id.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|source| PersistenceError::sqlite("read_setup_notices", source))?
        .map(|row| {
            let (kind, code, message) =
                row.map_err(|source| PersistenceError::sqlite("read_setup_notice", source))?;
            let warning = match kind.as_str() {
                "reason" => false,
                "warning" => true,
                _ => {
                    return Err(PersistenceError::InvalidRecord {
                        field: "setup_notice_kind",
                    });
                }
            };
            Ok(PersistedSetupNotice {
                warning,
                code,
                message,
            })
        })
        .collect()
}

fn read_plan_effects(
    connection: &rusqlite::Connection,
    job_id: &JobId,
) -> Result<Vec<String>, PersistenceError> {
    let mut statement = connection
        .prepare(
            "SELECT effect_code FROM setup_plan_effects
             WHERE job_id = ?1 ORDER BY ordinal",
        )
        .map_err(|source| PersistenceError::sqlite("prepare_setup_plan_effects", source))?;
    statement
        .query_map(params![job_id.as_str()], |row| row.get(0))
        .map_err(|source| PersistenceError::sqlite("read_setup_plan_effects", source))?
        .map(|row| row.map_err(|source| PersistenceError::sqlite("read_setup_plan_effect", source)))
        .collect()
}

fn read_approval_effects(
    connection: &rusqlite::Connection,
    job_id: &JobId,
) -> Result<Vec<String>, PersistenceError> {
    let mut statement = connection
        .prepare(
            "SELECT effect_code FROM setup_approval_effects
             WHERE job_id = ?1 ORDER BY ordinal",
        )
        .map_err(|source| PersistenceError::sqlite("prepare_setup_approval_effects", source))?;
    statement
        .query_map(params![job_id.as_str()], |row| row.get(0))
        .map_err(|source| PersistenceError::sqlite("read_setup_approval_effects", source))?
        .map(|row| {
            row.map_err(|source| PersistenceError::sqlite("read_setup_approval_effect", source))
        })
        .collect()
}

struct TransitionState {
    state: PersistedSetupState,
    revision: u64,
    event_sequence: u64,
    retry_count: u32,
    cancellation_requested: bool,
    cancelled_at_unix_ms: Option<i64>,
    progress: PersistedSetupProgress,
    runtime_verified_at_unix_ms: Option<i64>,
    model_available_at_unix_ms: Option<i64>,
    registration_verified_at_unix_ms: Option<i64>,
    inference_verified_at_unix_ms: Option<i64>,
    effect_count: u64,
}

fn read_transition_state(
    connection: &rusqlite::Connection,
    job_id: &JobId,
) -> Result<TransitionState, PersistenceError> {
    let raw = connection
        .query_row(
            "SELECT state, revision, last_event_sequence, retry_count,
                    cancellation_requested, cancelled_at_unix_ms,
                    progress_phase, progress_completed_bytes, progress_total_bytes,
                    progress_basis_points, runtime_verified_at_unix_ms,
                    model_available_at_unix_ms, registration_verified_at_unix_ms,
                    inference_verified_at_unix_ms,
                    (SELECT COUNT(*) FROM setup_effects WHERE setup_effects.job_id = setup_jobs.job_id)
             FROM setup_jobs WHERE job_id = ?1",
            params![job_id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, bool>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                    row.get::<_, Option<i64>>(9)?,
                    row.get::<_, Option<i64>>(10)?,
                    row.get::<_, Option<i64>>(11)?,
                    row.get::<_, Option<i64>>(12)?,
                    row.get::<_, Option<i64>>(13)?,
                    row.get::<_, i64>(14)?,
                ))
            },
        )
        .optional()
        .map_err(|source| PersistenceError::sqlite("read_setup_transition", source))?
        .ok_or(PersistenceError::RecordNotFound {
            entity: "setup_job",
        })?;
    let (
        state,
        revision,
        event_sequence,
        retry_count,
        cancellation_requested,
        cancelled_at_unix_ms,
        phase,
        completed,
        total,
        basis,
        runtime_verified_at_unix_ms,
        model_available_at_unix_ms,
        registration_verified_at_unix_ms,
        inference_verified_at_unix_ms,
        effect_count,
    ) = raw;
    Ok(TransitionState {
        state: PersistedSetupState::parse(&state)?,
        revision: to_u64(revision, "setup_revision")?,
        event_sequence: to_u64(event_sequence, "setup_event_sequence")?,
        retry_count: to_u32(retry_count, "setup_retry_count")?,
        cancellation_requested,
        cancelled_at_unix_ms,
        progress: PersistedSetupProgress {
            phase: phase.as_deref().map(parse_progress_phase).transpose()?,
            completed_bytes: optional_u64(completed, "setup_progress_completed")?,
            total_bytes: optional_u64(total, "setup_progress_total")?,
            basis_points: optional_u16(basis, "setup_progress_basis_points")?,
        }
        .validate()?,
        runtime_verified_at_unix_ms,
        model_available_at_unix_ms,
        registration_verified_at_unix_ms,
        inference_verified_at_unix_ms,
        effect_count: to_u64(effect_count, "setup_effect_count")?,
    })
}

fn next_progress(
    current: &PersistedSetupProgress,
    transition: &PersistedSetupTransition,
) -> Result<PersistedSetupProgress, PersistenceError> {
    if transition.clear_progress {
        return transition.progress.unwrap_or_default().validate();
    }
    let Some(next) = transition.progress else {
        return Ok(*current);
    };
    let next = next.validate()?;
    if matches!((current.completed_bytes, next.completed_bytes), (Some(old), Some(new)) if new < old)
        || matches!((current.total_bytes, next.total_bytes), (Some(old), Some(new)) if new != old)
        || matches!((current.basis_points, next.basis_points), (Some(old), Some(new)) if new < old)
    {
        return Err(PersistenceError::InvalidRecord {
            field: "setup_progress_regression",
        });
    }
    Ok(next)
}

fn next_evidence(current: Option<i64>, new: Option<i64>, clear: bool) -> Option<i64> {
    if clear { new } else { new.or(current) }
}

fn append_effects(
    connection: &rusqlite::Connection,
    transition: &PersistedSetupTransition,
    current_count: u64,
) -> Result<(), PersistenceError> {
    let final_count = current_count.saturating_add(transition.effects.len() as u64);
    if final_count > EFFECT_COUNT_MAX as u64 {
        return Err(PersistenceError::InvalidRecord {
            field: "setup_effect_count",
        });
    }
    for (index, effect) in transition.effects.iter().enumerate() {
        let ordinal = current_count.saturating_add(index as u64);
        connection
            .execute(
                "INSERT INTO setup_effects
                 (job_id, ordinal, disposition, effect_code, size_bytes, occurred_at_unix_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    transition.job_id.as_str(),
                    to_i64(ordinal, "setup_effect_ordinal")?,
                    effect.disposition.as_str(),
                    effect.effect_code,
                    effect
                        .size_bytes
                        .map(|value| to_i64(value, "setup_effect_size"))
                        .transpose()?,
                    transition.occurred_at_unix_ms,
                ],
            )
            .map_err(|source| PersistenceError::sqlite("write_setup_effect", source))?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_event(
    connection: &rusqlite::Connection,
    job_id: &JobId,
    sequence: i64,
    correlation_id: CorrelationId,
    kind: PersistedSetupEventKind,
    state: PersistedSetupState,
    stage: PersistedSetupStage,
    progress: PersistedSetupProgress,
    safe_code: Option<&str>,
    occurred_at_unix_ms: i64,
) -> Result<(), PersistenceError> {
    insert_event_on_connection(
        connection,
        job_id,
        sequence,
        correlation_id,
        kind,
        state,
        stage,
        progress,
        safe_code,
        occurred_at_unix_ms,
    )
}

#[allow(clippy::too_many_arguments)]
fn insert_event_on_connection(
    connection: &rusqlite::Connection,
    job_id: &JobId,
    sequence: i64,
    correlation_id: CorrelationId,
    kind: PersistedSetupEventKind,
    state: PersistedSetupState,
    stage: PersistedSetupStage,
    progress: PersistedSetupProgress,
    safe_code: Option<&str>,
    occurred_at_unix_ms: i64,
) -> Result<(), PersistenceError> {
    validate_timestamp(occurred_at_unix_ms)?;
    progress.validate()?;
    if let Some(code) = safe_code {
        validate_text("setup_event_code", code, ID_BYTES_MAX)?;
    }
    connection
        .execute(
            "INSERT INTO setup_job_events
             (job_id, sequence, correlation_id, kind, state, stage, progress_phase,
              progress_completed_bytes, progress_total_bytes,
              progress_basis_points, safe_code, occurred_at_unix_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                job_id.as_str(),
                sequence,
                correlation_id.to_string(),
                kind.as_str(),
                state.as_str(),
                stage.as_str(),
                progress.phase.map(progress_phase_as_str).transpose()?,
                progress
                    .completed_bytes
                    .map(|value| to_i64(value, "setup_progress_completed"))
                    .transpose()?,
                progress
                    .total_bytes
                    .map(|value| to_i64(value, "setup_progress_total"))
                    .transpose()?,
                progress.basis_points.map(i64::from),
                safe_code,
                occurred_at_unix_ms,
            ],
        )
        .map_err(|source| PersistenceError::sqlite("write_setup_event", source))?;
    Ok(())
}

fn prune_events(connection: &rusqlite::Connection, job_id: &JobId) -> Result<(), PersistenceError> {
    connection
        .execute(
            "DELETE FROM setup_job_events
             WHERE job_id = ?1 AND sequence NOT IN (
                 SELECT sequence FROM setup_job_events WHERE job_id = ?1
                 ORDER BY sequence DESC LIMIT ?2
             )",
            params![job_id.as_str(), i64::from(EVENT_RETENTION_MAX)],
        )
        .map_err(|source| PersistenceError::sqlite("prune_setup_events", source))?;
    Ok(())
}

fn validate_plan(plan: &PersistedSetupPlan) -> Result<(), PersistenceError> {
    if plan.plan_schema_version == 0
        || plan.plan_revision == 0
        || plan.expected_size_bytes > i64::MAX as u64
        || plan.expected_memory_bytes > i64::MAX as u64
        || plan.memory_safety_margin_bytes > i64::MAX as u64
        || plan.storage_safety_margin_bytes > i64::MAX as u64
        || plan
            .gpu_memory_bytes
            .is_some_and(|value| value > i64::MAX as u64)
        || [
            plan.observed_total_memory_bytes,
            plan.observed_available_memory_bytes,
            plan.observed_free_storage_bytes,
        ]
        .into_iter()
        .flatten()
        .any(|value| value > i64::MAX as u64)
        || plan.planned_context_tokens == 0
        || plan.notices.len() > NOTICE_COUNT_MAX
        || plan.effect_codes.len() > EFFECT_COUNT_MAX
    {
        return Err(PersistenceError::InvalidRecord {
            field: "setup_plan",
        });
    }
    validate_timestamp(plan.created_at_unix_ms)?;
    validate_text("setup_job_id", plan.job_id.as_str(), ID_BYTES_MAX)?;
    validate_text(
        "canonical_model_id",
        plan.canonical_model_id.as_str(),
        ID_BYTES_MAX,
    )?;
    validate_text(
        "model_display_name",
        &plan.model_display_name,
        DISPLAY_BYTES_MAX,
    )?;
    validate_text("model_family", &plan.model_family, ID_BYTES_MAX)?;
    model_size_class_as_str(plan.model_size_class)?;
    if let Some(acceleration) = plan.acceleration {
        acceleration_as_str(acceleration)?;
    }
    validate_text("licence_spdx", &plan.licence_spdx, ID_BYTES_MAX)?;
    validate_https_url(&plan.provenance_url)?;
    validate_text("provider_id", plan.provider_id.as_str(), ID_BYTES_MAX)?;
    validate_text(
        "provider_model_id",
        plan.provider_model_id.as_str(),
        PROVIDER_MODEL_ID_BYTES_MAX,
    )?;
    validate_safe_summary("artifact_source_summary", &plan.artifact_source_summary)?;
    validate_safe_summary("destination_display", &plan.destination_display)?;
    validate_text(
        "runtime_display_name",
        &plan.runtime_display_name,
        DISPLAY_BYTES_MAX,
    )?;
    if let Some(version) = &plan.runtime_version {
        validate_text("runtime_version", version, ID_BYTES_MAX)?;
    }
    validate_text(
        "catalogue_version",
        plan.catalogue_version.as_str(),
        ID_BYTES_MAX,
    )?;
    validate_text(
        "rule_set_version",
        plan.rule_set_version.as_str(),
        ID_BYTES_MAX,
    )?;
    for notice in &plan.notices {
        validate_text("setup_notice_code", &notice.code, ID_BYTES_MAX)?;
        validate_text("setup_notice_message", &notice.message, NOTICE_BYTES_MAX)?;
    }
    let mut effects = HashSet::new();
    for effect in &plan.effect_codes {
        validate_text("setup_effect_code", effect, ID_BYTES_MAX)?;
        if !effects.insert(effect.as_str()) {
            return Err(PersistenceError::InvalidRecord {
                field: "setup_effect_code",
            });
        }
    }
    Ok(())
}

fn validate_transition(transition: &PersistedSetupTransition) -> Result<(), PersistenceError> {
    validate_timestamp(transition.occurred_at_unix_ms)?;
    if transition.expected_revision > i64::MAX as u64 || transition.effects.len() > EFFECT_COUNT_MAX
    {
        return Err(PersistenceError::InvalidRecord {
            field: "setup_transition",
        });
    }
    if let Some(progress) = transition.progress {
        progress.validate()?;
    }
    for value in [
        transition.safe_error_code.as_deref(),
        transition.attention_reason.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        validate_text("setup_safe_code", value, ID_BYTES_MAX)?;
    }
    for effect in &transition.effects {
        validate_text("setup_effect_code", &effect.effect_code, ID_BYTES_MAX)?;
        if effect
            .size_bytes
            .is_some_and(|value| value > i64::MAX as u64)
        {
            return Err(PersistenceError::InvalidRecord {
                field: "setup_effect_size",
            });
        }
    }
    if let Some(update) = &transition.artifact_update {
        validate_artifact_update(update)?;
    }
    Ok(())
}

fn validate_artifact_update(update: &PersistedArtifactUpdate) -> Result<(), PersistenceError> {
    if update
        .measured_size_bytes
        .is_some_and(|value| value > i64::MAX as u64)
        || (update.lifecycle == PersistedModelLifecycle::Available
            && (update.verification != PersistedModelVerification::Verified
                || !matches!(
                    update.integrity,
                    PersistedModelIntegrity::ProviderReported | PersistedModelIntegrity::Verified
                )
                || update.registered_at_unix_ms.is_none()
                || update.verified_at_unix_ms.is_none()))
    {
        return Err(PersistenceError::InvalidRecord {
            field: "model_artifact_update",
        });
    }
    for timestamp in [update.registered_at_unix_ms, update.verified_at_unix_ms]
        .into_iter()
        .flatten()
    {
        validate_timestamp(timestamp)?;
    }
    Ok(())
}

const fn durable_job_state(state: PersistedSetupState) -> &'static str {
    match state {
        PersistedSetupState::DraftPlan
        | PersistedSetupState::AwaitingApproval
        | PersistedSetupState::Approved => "queued",
        PersistedSetupState::Preparing
        | PersistedSetupState::CheckingStorage
        | PersistedSetupState::Acquiring
        | PersistedSetupState::Registering
        | PersistedSetupState::VerifyingRuntime
        | PersistedSetupState::VerifyingModel
        | PersistedSetupState::RunningTestInference => "running",
        PersistedSetupState::Ready => "succeeded",
        PersistedSetupState::AttentionRequired | PersistedSetupState::Failed => "failed",
        PersistedSetupState::Cancelled => "cancelled",
    }
}

fn progress_phase_as_str(value: ModelAcquisitionPhase) -> Result<&'static str, PersistenceError> {
    match value {
        ModelAcquisitionPhase::Preparing => Ok("preparing"),
        ModelAcquisitionPhase::Transferring => Ok("transferring"),
        ModelAcquisitionPhase::Verifying => Ok("verifying"),
        ModelAcquisitionPhase::Registering => Ok("registering"),
        ModelAcquisitionPhase::Completed => Ok("completed"),
        ModelAcquisitionPhase::Unknown => Err(PersistenceError::InvalidRecord {
            field: "setup_progress_phase",
        }),
        _ => Err(PersistenceError::InvalidRecord {
            field: "setup_progress_phase",
        }),
    }
}

fn parse_progress_phase(value: &str) -> Result<ModelAcquisitionPhase, PersistenceError> {
    match value {
        "preparing" => Ok(ModelAcquisitionPhase::Preparing),
        "transferring" => Ok(ModelAcquisitionPhase::Transferring),
        "verifying" => Ok(ModelAcquisitionPhase::Verifying),
        "registering" => Ok(ModelAcquisitionPhase::Registering),
        "completed" => Ok(ModelAcquisitionPhase::Completed),
        _ => Err(PersistenceError::InvalidRecord {
            field: "setup_progress_phase",
        }),
    }
}

fn acceleration_as_str(value: AccelerationKind) -> Result<&'static str, PersistenceError> {
    match value {
        AccelerationKind::DirectMl => Ok("direct_ml"),
        AccelerationKind::Cuda => Ok("cuda"),
        AccelerationKind::Rocm => Ok("rocm"),
        AccelerationKind::Unknown => Err(PersistenceError::InvalidRecord {
            field: "setup_acceleration",
        }),
        _ => Err(PersistenceError::InvalidRecord {
            field: "setup_acceleration",
        }),
    }
}

fn parse_acceleration(value: &str) -> Result<AccelerationKind, PersistenceError> {
    match value {
        "direct_ml" => Ok(AccelerationKind::DirectMl),
        "cuda" => Ok(AccelerationKind::Cuda),
        "rocm" => Ok(AccelerationKind::Rocm),
        _ => Err(PersistenceError::InvalidRecord {
            field: "setup_acceleration",
        }),
    }
}

fn model_size_class_as_str(value: ModelSizeClass) -> Result<&'static str, PersistenceError> {
    match value {
        ModelSizeClass::Compact => Ok("compact"),
        ModelSizeClass::Standard => Ok("standard"),
        ModelSizeClass::Large => Ok("large"),
        ModelSizeClass::Unknown => Err(PersistenceError::InvalidRecord {
            field: "model_size_class",
        }),
        _ => Err(PersistenceError::InvalidRecord {
            field: "model_size_class",
        }),
    }
}

fn parse_model_size_class(value: &str) -> Result<ModelSizeClass, PersistenceError> {
    match value {
        "compact" => Ok(ModelSizeClass::Compact),
        "standard" => Ok(ModelSizeClass::Standard),
        "large" => Ok(ModelSizeClass::Large),
        _ => Err(PersistenceError::InvalidRecord {
            field: "model_size_class",
        }),
    }
}

fn validate_https_url(value: &str) -> Result<(), PersistenceError> {
    validate_text("provenance_url", value, URL_BYTES_MAX)?;
    if value.starts_with("https://")
        && !value
            .chars()
            .any(|character| matches!(character, '\r' | '\n'))
    {
        Ok(())
    } else {
        Err(PersistenceError::InvalidRecord {
            field: "provenance_url",
        })
    }
}

fn validate_safe_summary(field: &'static str, value: &str) -> Result<(), PersistenceError> {
    validate_text(field, value, DISPLAY_BYTES_MAX)?;
    if value
        .chars()
        .any(|character| matches!(character, '/' | '\\'))
    {
        Err(PersistenceError::InvalidRecord { field })
    } else {
        Ok(())
    }
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

fn to_i64(value: u64, field: &'static str) -> Result<i64, PersistenceError> {
    i64::try_from(value).map_err(|_| PersistenceError::InvalidRecord { field })
}

fn to_u64(value: i64, field: &'static str) -> Result<u64, PersistenceError> {
    u64::try_from(value).map_err(|_| PersistenceError::InvalidRecord { field })
}

fn to_u32(value: i64, field: &'static str) -> Result<u32, PersistenceError> {
    u32::try_from(value).map_err(|_| PersistenceError::InvalidRecord { field })
}

fn optional_u64(value: Option<i64>, field: &'static str) -> Result<Option<u64>, PersistenceError> {
    value.map(|inner| to_u64(inner, field)).transpose()
}

fn optional_u16(value: Option<i64>, field: &'static str) -> Result<Option<u16>, PersistenceError> {
    value
        .map(|inner| u16::try_from(inner).map_err(|_| PersistenceError::InvalidRecord { field }))
        .transpose()
}

fn checked_increment(value: i64, field: &'static str) -> Result<i64, PersistenceError> {
    value
        .checked_add(1)
        .ok_or(PersistenceError::InvalidRecord { field })
}
