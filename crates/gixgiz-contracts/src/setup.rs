use std::{fmt, str::FromStr};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    CandidateModelId, CatalogueVersion, CorrelationId, ModelSizeClass, RecommendationPlan,
    RequestId, ResourceEstimate, RuleSetVersion, RuntimeProviderId, RuntimeProviderModelId,
    SafeErrorPayload,
};

/// Schema generation for the persistent model-setup workflow.
pub const SETUP_WORKFLOW_SCHEMA_VERSION: u32 = 1;

/// Opaque identifier for one durable setup job.
#[derive(Clone, Copy, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SetupJobId(Uuid);

impl SetupJobId {
    /// Generates a random opaque version-four identifier.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates an identifier from a UUID for deterministic callers.
    #[must_use]
    pub const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    /// Returns the underlying UUID value.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for SetupJobId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SetupJobId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for SetupJobId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}

/// High-level durable setup-job state.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum SetupJobState {
    /// A plan is being assembled and has no approval.
    DraftPlan,
    /// The exact persisted plan requires a user decision.
    AwaitingApproval,
    /// The exact current plan revision was approved.
    Approved,
    /// Bounded setup work is currently active.
    Active,
    /// Work paused because a recoverable condition needs attention.
    AttentionRequired,
    /// Runtime, model, registration, and inference verification all passed.
    Ready,
    /// The current attempt failed safely.
    Failed,
    /// Explicit cancellation was observed.
    Cancelled,
    /// A newer peer supplied an unrecognized state.
    #[serde(other)]
    Unknown,
}

/// Exact stage owned by the persistent setup state machine.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum SetupStage {
    /// A concrete plan is being prepared.
    DraftPlan,
    /// No provider mutation is permitted until approval is recorded.
    AwaitingApproval,
    /// Approval is recorded and execution has not started.
    Approved,
    /// The provider and durable job are being prepared.
    Preparing,
    /// Destination availability and capacity are being checked.
    CheckingStorage,
    /// The approved provider artifact is being acquired.
    Acquiring,
    /// Provider registration is being completed or observed.
    Registering,
    /// Runtime health is being reverified.
    VerifyingRuntime,
    /// Model availability and registration are being verified.
    VerifyingModel,
    /// A fixed bounded non-private test inference is running.
    RunningTestInference,
    /// Every readiness gate passed.
    Ready,
    /// A recoverable condition requires user attention.
    AttentionRequired,
    /// The current attempt failed safely.
    Failed,
    /// Explicit cancellation was observed.
    Cancelled,
    /// A newer peer supplied an unrecognized stage.
    #[serde(other)]
    Unknown,
}

/// Terminal outcome for one bounded setup attempt or event stream.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum SetupJobTerminalState {
    /// Every required readiness gate passed.
    Ready,
    /// The attempt paused with an actionable recovery path.
    AttentionRequired,
    /// The attempt failed safely.
    Failed,
    /// Explicit cancellation was observed.
    Cancelled,
    /// A newer peer supplied an unrecognized terminal state.
    #[serde(other)]
    Unknown,
}

/// Stable reason why a setup job needs attention.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum SetupAttentionReason {
    /// No compatible runtime is installed.
    RuntimeNotInstalled,
    /// Runtime reuse consent has not been recorded.
    RuntimeConsentRequired,
    /// The compatible runtime is stopped or unavailable.
    RuntimeUnavailable,
    /// Runtime installation would require a separately approved privileged workflow.
    PrivilegedRuntimeInstallationRequired,
    /// The approved destination is missing or cannot be verified.
    DestinationUnavailable,
    /// The destination does not retain the required safety margin.
    InsufficientStorage,
    /// Provider acquisition was interrupted with uncertain retained effects.
    AcquisitionInterrupted,
    /// Provider registration could not be verified.
    RegistrationUnverified,
    /// The selected model is not available through the provider.
    ModelUnavailable,
    /// Available integrity evidence did not match.
    IntegrityMismatch,
    /// Readiness inference exceeded its fixed deadline.
    ReadinessTimedOut,
    /// Readiness inference failed without exposing generated content.
    ReadinessFailed,
    /// An interrupted job requires an explicit retry decision.
    RecoveryRequired,
    /// A newer peer supplied an unrecognized reason.
    #[serde(other)]
    Unknown,
}

/// Safe recovery action offered for a setup state.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum SetupRecoveryAction {
    /// Review and approve the exact current plan revision.
    ReviewApproval,
    /// Retry the same approved job after rechecking preconditions.
    Retry,
    /// Restore or reconnect the approved destination.
    RestoreDestination,
    /// Free storage while preserving the required safety margin.
    FreeStorage,
    /// Restore the compatible external runtime outside GixGiz.
    RestoreRuntime,
    /// Review runtime and platform prerequisites.
    CheckPrerequisites,
    /// Escalate with bounded sanitized diagnostics.
    ContactSupport,
    /// No action is required.
    NoAction,
    /// A newer peer supplied an unrecognized action.
    #[serde(other)]
    Unknown,
}

/// Category of a concrete plan component.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum SetupPlanComponentKind {
    /// Runtime reuse and health requirements.
    Runtime,
    /// Canonical model and provider artifact.
    Model,
    /// Provider-managed or GixGiz-managed storage destination.
    Storage,
    /// Readiness verification work.
    Verification,
    /// A newer peer supplied an unrecognized kind.
    #[serde(other)]
    Unknown,
}

/// Safe destination category shown without exposing a private local path.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum SetupDestinationCategory {
    /// Storage is owned and selected by the runtime provider.
    ProviderManaged,
    /// Storage is beneath the GixGiz per-user application root.
    ApplicationData,
    /// Storage is beneath an explicitly approved user-selected root.
    UserSelected,
    /// Destination ownership or location cannot be established safely.
    #[serde(other)]
    Unknown,
}

/// Stable plan warning category.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum SetupWarningCode {
    /// The selected runtime remains externally managed.
    ExternalRuntimeModified,
    /// Storage remains owned and controlled by the provider.
    ProviderManagedStorage,
    /// No trusted expected checksum is available for independent comparison.
    IntegrityMetadataUnavailable,
    /// Cancellation may retain provider-owned partial data.
    CancellationMayRetainEffects,
    /// Some destination evidence remains explicitly uncertain.
    DestinationEvidenceIncomplete,
    /// A newer peer supplied an unrecognized warning.
    #[serde(other)]
    Unknown,
}

/// Safe bounded setup-plan warning.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupWarning {
    /// Stable warning category.
    pub code: SetupWarningCode,
    /// Provider-neutral explanation safe for display.
    pub message: String,
}

/// Stable plan or transition reason category.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum SetupReasonCode {
    /// The model came from the deterministic capability recommendation.
    RecommendationSelected,
    /// An existing compatible provider artifact may be reused.
    ExistingModelReusable,
    /// Acquisition is required for the selected provider artifact.
    AcquisitionRequired,
    /// Exact approval is required before provider mutation.
    ApprovalRequired,
    /// Storage preflight passed with the required safety margin.
    StorageVerified,
    /// Runtime health was verified.
    RuntimeVerified,
    /// Provider registration was verified.
    RegistrationVerified,
    /// Bounded readiness inference was verified.
    ReadinessVerified,
    /// A newer peer supplied an unrecognized reason.
    #[serde(other)]
    Unknown,
}

/// Safe bounded setup reason.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupReason {
    /// Stable reason category.
    pub code: SetupReasonCode,
    /// Provider-neutral explanation safe for display.
    pub message: String,
}

/// One concrete, reviewable plan component.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupPlanComponent {
    /// Component category.
    pub kind: SetupPlanComponentKind,
    /// Bounded user-facing component name.
    pub title: String,
    /// Bounded user-facing component detail.
    pub detail: String,
    /// Whether setup cannot become ready without this component.
    pub required: bool,
}

/// Opaque provider artifact mapped from one canonical catalogue identity.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ModelProviderArtifact {
    /// Provider-neutral canonical catalogue identity.
    pub canonical_model_id: CandidateModelId,
    /// Runtime provider that owns the mapping.
    pub provider_id: RuntimeProviderId,
    /// Opaque provider identifier required for adapter operations.
    pub provider_model_id: RuntimeProviderModelId,
    /// Bounded safe source or mapping summary.
    pub source_summary: String,
}

/// Persistent lifecycle state for one canonical model mapping.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ModelLifecycleState {
    /// The model is part of a concrete plan only.
    Planned,
    /// Provider acquisition is active or incomplete.
    Acquiring,
    /// Acquisition completed but registration/readiness is not complete.
    Acquired,
    /// Provider registration is being observed.
    Registering,
    /// Runtime, availability, registration, or inference verification is active.
    Verifying,
    /// Every required model-readiness gate passed.
    Available,
    /// A recoverable condition requires attention.
    AttentionRequired,
    /// Model setup failed safely.
    Failed,
    /// Explicit cancellation was observed before readiness.
    Cancelled,
    /// A newer peer supplied an unrecognized state.
    #[serde(other)]
    Unknown,
}

/// Aggregate verification state for one model setup.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ModelVerificationState {
    /// No verification gate has run.
    NotStarted,
    /// Runtime health passed and later gates remain.
    RuntimeVerified,
    /// Model availability passed and later gates remain.
    AvailabilityVerified,
    /// Provider registration passed and inference remains.
    RegistrationVerified,
    /// Every required verification gate passed.
    Verified,
    /// One verification gate failed safely.
    Failed,
    /// Verification was cancelled.
    Cancelled,
    /// A newer peer supplied an unrecognized state.
    #[serde(other)]
    Unknown,
}

/// Strength of available artifact-integrity evidence.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ModelIntegrityState {
    /// No trusted expected identity is available for independent comparison.
    Unavailable,
    /// The provider reported internal verification without an independent expectation.
    ProviderReported,
    /// Expected and observed integrity evidence matched.
    Verified,
    /// Expected and observed integrity evidence did not match.
    Mismatch,
    /// A newer peer supplied an unrecognized state.
    #[serde(other)]
    Unknown,
}

/// Normalized provider acquisition phase.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ModelAcquisitionPhase {
    /// Provider preparation or manifest resolution is active.
    Preparing,
    /// Artifact bytes are being transferred.
    Transferring,
    /// Provider-reported integrity work is active.
    Verifying,
    /// Provider registration or manifest commit is active.
    Registering,
    /// Provider acquisition reported success.
    Completed,
    /// A newer peer supplied an unrecognized phase.
    #[serde(other)]
    Unknown,
}

/// Bounded normalized progress with no provider-supplied text.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ModelAcquisitionProgress {
    /// Current normalized acquisition phase.
    pub phase: ModelAcquisitionPhase,
    /// Completed bytes when the provider supplied valid counters.
    pub completed_bytes: Option<u64>,
    /// Total bytes when the provider supplied valid counters.
    pub total_bytes: Option<u64>,
    /// Deterministic progress in hundredths of one percent, when calculable.
    pub progress_basis_points: Option<u16>,
}

/// Provider registration state for one exact artifact.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ProviderRegistrationState {
    /// Registration has not been checked.
    NotChecked,
    /// The exact provider artifact is registered locally.
    Registered,
    /// The exact provider artifact is not registered locally.
    NotRegistered,
    /// Registration could not be established or was supplied by a newer peer.
    #[serde(other)]
    Unknown,
}

/// Bounded provider-registration evidence.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ProviderRegistrationResult {
    /// Registration result.
    pub state: ProviderRegistrationState,
    /// Provider-reported installed size when valid.
    pub measured_size_bytes: Option<u64>,
    /// UTC Unix timestamp in milliseconds captured after inspection.
    pub verified_at_unix_ms: u64,
}

/// Aggregate readiness evidence without prompt or generated content.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ModelVerificationResult {
    /// Whether compatible runtime health was verified.
    pub runtime_health_verified: bool,
    /// Whether the exact local provider artifact was available.
    pub model_available: bool,
    /// Whether provider registration was verified.
    pub registration_verified: bool,
    /// Whether a fixed bounded readiness inference produced a valid response.
    pub inference_verified: bool,
    /// Available integrity evidence classification.
    pub integrity: ModelIntegrityState,
    /// UTC Unix timestamp in milliseconds captured after the latest gate.
    pub verified_at_unix_ms: u64,
}

/// Persistable provider-neutral model metadata.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ModelMetadata {
    /// Setup-workflow schema generation.
    pub schema_version: u32,
    /// Canonical/provider identity mapping.
    pub artifact: ModelProviderArtifact,
    /// Beginner-readable model name.
    pub display_name: String,
    /// Canonical model family.
    pub family: String,
    /// Coarse model size class.
    pub size_class: ModelSizeClass,
    /// SPDX licence recorded by the local catalogue.
    pub licence_spdx: String,
    /// Bounded provenance summary or official source URL.
    pub provenance: String,
    /// Safe destination ownership category.
    pub destination: SetupDestinationCategory,
    /// Safe destination label with no private local path.
    pub destination_display: String,
    /// Conservative expected installed size.
    pub expected_size_bytes: u64,
    /// Provider-reported measured installed size when available.
    pub measured_size_bytes: Option<u64>,
    /// Catalogue version used to select this model.
    pub catalogue_version: CatalogueVersion,
    /// Rule-set version used to select this model.
    pub rule_set_version: RuleSetVersion,
    /// Persistent model lifecycle state.
    pub lifecycle_state: ModelLifecycleState,
    /// Aggregate model verification state.
    pub verification_state: ModelVerificationState,
    /// Latest bounded verification evidence when available.
    pub verification: Option<ModelVerificationResult>,
}

/// Material setup effect category.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum SetupEffectKind {
    /// Add provider-owned model data.
    ProviderModelAcquisition,
    /// Register or observe the exact model through the provider.
    ProviderModelRegistration,
    /// Run one fixed bounded readiness inference.
    ReadinessInference,
    /// Persist GixGiz-owned job and model metadata.
    MetadataPersistence,
    /// A newer peer supplied an unrecognized effect.
    #[serde(other)]
    Unknown,
}

/// Outcome classification for one material effect.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum SetupEffectDisposition {
    /// The effect completed and was verified.
    Completed,
    /// The effect remains intentionally retained.
    Retained,
    /// The workflow safely reversed its owned effect.
    RolledBack,
    /// The provider effect cannot be established safely.
    Uncertain,
    /// A newer peer supplied an unrecognized disposition.
    #[serde(other)]
    Unknown,
}

/// One bounded material-effect record.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupEffect {
    /// Effect category.
    pub kind: SetupEffectKind,
    /// Observed effect disposition.
    pub disposition: SetupEffectDisposition,
    /// Safe bounded explanation.
    pub message: String,
}

/// Bounded summary of material effects.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupEffectReport {
    /// Ordered material-effect records.
    pub effects: Vec<SetupEffect>,
}

/// Structured cancellation result that does not conflate cancellation with failure.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupCancellationReport {
    /// UTC Unix timestamp in milliseconds when cancellation was requested.
    pub requested_at_unix_ms: u64,
    /// UTC Unix timestamp in milliseconds when the workflow observed cancellation.
    pub observed_at_unix_ms: u64,
    /// Completed, retained, rolled-back, and uncertain effects.
    pub effect_report: SetupEffectReport,
}

/// Concrete setup plan derived from one deterministic recommendation.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupPlan {
    /// Setup-workflow schema generation.
    pub schema_version: u32,
    /// Durable job that owns this plan.
    pub job_id: SetupJobId,
    /// Monotonic plan revision used to bind approval.
    pub revision: u32,
    /// Selected model metadata before acquisition.
    pub model: ModelMetadata,
    /// Beginner-readable runtime name.
    pub runtime_display_name: String,
    /// Verified provider version when available.
    pub runtime_version: Option<String>,
    /// Conservative resource expectations copied from the recommendation.
    pub resources: ResourceEstimate,
    /// Concrete reviewable components.
    pub components: Vec<SetupPlanComponent>,
    /// Material effects that exact approval authorizes.
    pub required_effects: Vec<SetupEffectKind>,
    /// Safe deterministic plan reasons.
    pub reasons: Vec<SetupReason>,
    /// Safe plan warnings and limitations.
    pub warnings: Vec<SetupWarning>,
}

/// Explicit decision for one exact setup-plan revision.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum SetupApprovalDecision {
    /// Approve only the exact persisted plan revision and its recorded effects.
    Approve,
    /// Deny the current plan revision.
    Deny,
    /// A newer client supplied an unrecognized decision.
    #[serde(other)]
    Unknown,
}

/// Durable approval record created by Rust after validating the exact plan revision.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupApprovalRecord {
    /// Approved durable job.
    pub job_id: SetupJobId,
    /// Approved plan revision.
    pub plan_revision: u32,
    /// Explicit user decision.
    pub decision: SetupApprovalDecision,
    /// Approved canonical model.
    pub canonical_model_id: CandidateModelId,
    /// Approved runtime provider.
    pub provider_id: RuntimeProviderId,
    /// Approved provider artifact.
    pub provider_model_id: RuntimeProviderModelId,
    /// Approved destination category.
    pub destination: SetupDestinationCategory,
    /// Approved conservative size expectation.
    pub expected_size_bytes: u64,
    /// SPDX licence copied from the persisted model plan.
    pub licence_spdx: String,
    /// Bounded source provenance copied from the persisted model plan.
    pub provenance: String,
    /// Approved material effects.
    pub approved_effects: Vec<SetupEffectKind>,
    /// Whether this approval permits an effect on an externally managed runtime.
    pub external_runtime_effect: bool,
    /// Correlation identifier from the approval request.
    pub correlation_id: CorrelationId,
    /// Identifier of the approval request.
    pub request_id: RequestId,
    /// UTC Unix timestamp in milliseconds assigned by Rust.
    pub decided_at_unix_ms: u64,
}

/// Authoritative persistent setup-job snapshot.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupJobSnapshot {
    /// Setup-workflow schema generation.
    pub schema_version: u32,
    /// Durable job identifier.
    pub job_id: SetupJobId,
    /// Exact current plan.
    pub plan: SetupPlan,
    /// High-level durable state.
    pub state: SetupJobState,
    /// Exact current state-machine stage.
    pub stage: SetupStage,
    /// Latest persisted normalized acquisition progress.
    pub progress: Option<ModelAcquisitionProgress>,
    /// Current attention reason when action is required.
    pub attention_reason: Option<SetupAttentionReason>,
    /// Recommended recovery action.
    pub recovery_action: Option<SetupRecoveryAction>,
    /// Latest safe boundary error.
    pub error: Option<SafeErrorPayload>,
    /// Latest bounded material-effect report.
    pub effect_report: Option<SetupEffectReport>,
    /// Structured report after cancellation.
    pub cancellation_report: Option<SetupCancellationReport>,
    /// Whether cooperative cancellation is currently requested.
    pub cancellation_requested: bool,
    /// Number of explicit retry attempts.
    pub retry_count: u32,
    /// Latest durable event sequence included by this authoritative snapshot.
    ///
    /// Clients resume event observation strictly after this cursor. This remains valid when
    /// older coalesced progress events have been removed from bounded retention.
    pub latest_event_sequence: u64,
    /// UTC Unix timestamp in milliseconds when the job was created.
    pub created_at_unix_ms: u64,
    /// UTC Unix timestamp in milliseconds when the snapshot last changed.
    pub updated_at_unix_ms: u64,
}

/// Authenticated request for a concrete setup plan.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupPlanRequest {
    /// Deterministic recommendation selected for setup and revalidated by Rust.
    pub recommendation: RecommendationPlan,
    /// Registered runtime provider selected for the recommendation.
    pub provider_id: RuntimeProviderId,
    /// Requested safe destination category.
    pub destination: SetupDestinationCategory,
    /// Identifier shared with the response and safe failures.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Concrete plan plus its durable awaiting-approval job.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupPlanResponse {
    /// Authoritative durable job snapshot.
    pub job: SetupJobSnapshot,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Authenticated exact setup approval request.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupApprovalRequest {
    /// Durable job being approved or denied.
    pub job_id: SetupJobId,
    /// Exact plan revision displayed to the user.
    pub plan_revision: u32,
    /// Explicit approval decision.
    pub decision: SetupApprovalDecision,
    /// Identifier shared with the response and safe failures.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Authoritative job and durable approval outcome.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupApprovalResponse {
    /// Authoritative durable job snapshot after the decision.
    pub job: SetupJobSnapshot,
    /// Durable decision record.
    pub approval: SetupApprovalRecord,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Authenticated request to start one approved setup attempt.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupJobStartRequest {
    /// Approved durable job.
    pub job_id: SetupJobId,
    /// Exact approved plan revision.
    pub plan_revision: u32,
    /// Identifier shared by this attempt and its events.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Accepted persistent setup attempt.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupJobStartResponse {
    /// Authoritative durable job snapshot after acceptance.
    pub job: SetupJobSnapshot,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Authenticated request for one durable setup-job snapshot.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupJobStatusRequest {
    /// Requested durable job.
    pub job_id: SetupJobId,
    /// Identifier shared with the response and safe failures.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Authoritative durable setup-job status.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupJobStatusResponse {
    /// Current persisted job snapshot.
    pub job: SetupJobSnapshot,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Authenticated request to recover the most recent durable setup job.
///
/// Recovery is intentionally not keyed by an attempt correlation identifier. The core selects
/// the latest persisted job that still matters to the current installation, including terminal
/// jobs whose outcome the user has not yet acknowledged.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupJobRecoveryRequest {
    /// Identifier shared with the response and safe failures.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Latest durable setup state available after desktop or core restart.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupJobRecoveryResponse {
    /// Most recent relevant job, or `None` when setup has never been planned.
    pub job: Option<SetupJobSnapshot>,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Authenticated cooperative cancellation request for a setup job.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupJobCancelRequest {
    /// Target durable job.
    pub job_id: SetupJobId,
    /// Identifier shared with the attempt being cancelled.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Cancellation acceptance and latest persistent state.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupJobCancelResponse {
    /// Whether cancellation was accepted for an active non-terminal attempt.
    pub accepted: bool,
    /// Current authoritative job snapshot.
    pub job: SetupJobSnapshot,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Authenticated request to retry the exact current approved plan.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupJobRetryRequest {
    /// Durable job to retry.
    pub job_id: SetupJobId,
    /// Exact approved plan revision.
    pub plan_revision: u32,
    /// Identifier shared by the new attempt and its events.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Accepted idempotent retry attempt.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupJobRetryResponse {
    /// Current authoritative job snapshot.
    pub job: SetupJobSnapshot,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Stable setup-job event classification.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum SetupJobEventKind {
    /// A bounded setup attempt started.
    Started,
    /// Persisted setup progress changed.
    Progress,
    /// The attempt paused for attention.
    AttentionRequired,
    /// Every readiness gate passed.
    Ready,
    /// The attempt failed safely.
    Failed,
    /// Explicit cancellation was observed.
    Cancelled,
    /// A newer peer supplied an unrecognized event kind.
    #[serde(other)]
    Unknown,
}

/// Ordered persisted setup-job event suitable for bounded replay.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupJobEvent {
    /// Setup-workflow schema generation.
    pub schema_version: u32,
    /// Durable job that owns this event.
    pub job_id: SetupJobId,
    /// Identifier of the request or bounded attempt that caused this event.
    pub correlation_id: CorrelationId,
    /// Per-job monotonically increasing sequence number starting at one and continuing across
    /// retries and process restarts.
    pub sequence: u64,
    /// Stable event kind.
    pub kind: SetupJobEventKind,
    /// UTC Unix timestamp in milliseconds assigned by Rust.
    pub timestamp_unix_ms: u64,
    /// Authoritative persisted snapshot at this event.
    pub job: SetupJobSnapshot,
    /// Terminal outcome present only on the final event for an attempt.
    pub terminal_state: Option<SetupJobTerminalState>,
}

/// Authenticated bounded replay request for persisted setup-job events.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupJobEventsRequest {
    /// Durable job whose events are requested.
    pub job_id: SetupJobId,
    /// Exclusive durable sequence cursor. Zero starts replay at the first event.
    pub after_sequence: u64,
    /// Requested maximum event count. Rust applies its own lower hard upper bound.
    pub limit: u32,
    /// Identifier shared with the response and safe failures.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// One bounded page of durable setup-job events.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SetupJobEventsResponse {
    /// Events in strictly increasing durable sequence order.
    pub events: Vec<SetupJobEvent>,
    /// Cursor to send as `after_sequence` for the next page.
    pub next_after_sequence: u64,
    /// Whether additional persisted events exist after this page.
    pub has_more: bool,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_enums_are_forward_compatible_without_becoming_ready() {
        let stage: SetupStage =
            serde_json::from_str("\"future_stage\"").expect("future stage accepted");
        let lifecycle: ModelLifecycleState =
            serde_json::from_str("\"future_state\"").expect("future lifecycle accepted");
        let phase: ModelAcquisitionPhase =
            serde_json::from_str("\"future_phase\"").expect("future phase accepted");

        assert_eq!(stage, SetupStage::Unknown);
        assert_eq!(lifecycle, ModelLifecycleState::Unknown);
        assert_eq!(phase, ModelAcquisitionPhase::Unknown);
    }

    #[test]
    fn setup_job_id_round_trips_as_an_opaque_uuid() {
        let original = SetupJobId::from_uuid(Uuid::nil());
        let encoded = serde_json::to_string(&original).expect("job id serializes");
        let decoded: SetupJobId = serde_json::from_str(&encoded).expect("job id deserializes");

        assert_eq!(decoded, original);
        assert_eq!(original.to_string(), "00000000-0000-0000-0000-000000000000");
    }

    #[test]
    fn setup_schema_has_no_provider_payload_prompt_output_or_private_path() {
        let schema = serde_json::to_string(&schemars::schema_for!(SetupJobSnapshot))
            .expect("setup schema serializes")
            .to_ascii_lowercase();

        for forbidden in [
            "ollama",
            "/api/",
            "raw_payload",
            "provider_logs",
            "prompt_text",
            "generated_text",
            "local_path",
            "digest",
        ] {
            assert!(!schema.contains(forbidden), "schema contains {forbidden}");
        }
    }

    #[test]
    fn cancellation_effect_dispositions_remain_explicit() {
        let report = SetupCancellationReport {
            requested_at_unix_ms: 1,
            observed_at_unix_ms: 2,
            effect_report: SetupEffectReport {
                effects: vec![
                    SetupEffect {
                        kind: SetupEffectKind::MetadataPersistence,
                        disposition: SetupEffectDisposition::Completed,
                        message: "Job metadata was retained.".to_owned(),
                    },
                    SetupEffect {
                        kind: SetupEffectKind::ProviderModelAcquisition,
                        disposition: SetupEffectDisposition::Uncertain,
                        message: "Provider-owned partial data may remain.".to_owned(),
                    },
                ],
            },
        };

        let json = serde_json::to_string(&report).expect("report serializes");
        assert!(json.contains("\"completed\""));
        assert!(json.contains("\"uncertain\""));
    }
}
