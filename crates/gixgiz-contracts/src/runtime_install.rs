//! Provider-neutral contracts for approved local runtime installation.
//!
//! These types describe *what* a runtime installation is, never *how* one
//! provider performs it. Installer executables, arguments, registry keys, and
//! download URLs are adapter details and never appear here.
//!
//! Two approvals are deliberately distinct:
//!
//! - a GixGiz plan approval, recorded here, is the user accepting one exact
//!   revision of one described installation; and
//! - any operating-system elevation prompt, which is not represented in these
//!   contracts at all and can never substitute for a plan approval.

use std::{fmt, str::FromStr};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    CorrelationId, RequestId, RuntimeDisplayName, RuntimeOwnership, RuntimeProviderId,
    SafeErrorPayload,
};

/// Schema generation for the runtime-installation workflow.
pub const RUNTIME_INSTALL_SCHEMA_VERSION: u32 = 1;

/// Maximum installer artifact size any plan may declare.
pub const MAX_INSTALLER_ARTIFACT_BYTES: u64 = 4 * 1024 * 1024 * 1024;

/// Maximum bounded install-job events returned in one page.
pub const MAX_INSTALL_EVENT_PAGE: u16 = 200;

/// Opaque identifier for one durable runtime-installation job.
#[derive(Clone, Copy, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RuntimeInstallJobId(Uuid);

impl RuntimeInstallJobId {
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

impl Default for RuntimeInstallJobId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for RuntimeInstallJobId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for RuntimeInstallJobId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}

/// High-level durable runtime-installation job state.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeInstallState {
    /// A plan exists and has never been approved.
    AwaitingApproval,
    /// The exact current plan revision was approved and work may begin.
    Approved,
    /// Approved work is in progress.
    Running,
    /// Work stopped and needs an explicit user decision.
    AttentionRequired,
    /// The runtime was installed and independently verified.
    Ready,
    /// Work stopped without a verified runtime.
    Failed,
    /// The user cancelled or denied the plan.
    Cancelled,
    /// A newer peer supplied an unrecognized state.
    #[serde(other)]
    Unknown,
}

impl RuntimeInstallState {
    /// Returns whether no further transition can occur without a new plan.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Ready | Self::Failed | Self::Cancelled)
    }
}

/// Ordered stage within an approved runtime installation.
///
/// Stages are observable progress only. They never imply success: a stage may
/// be reached and then fail, and only `Ready` follows full verification.
#[derive(
    Clone, Copy, Debug, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeInstallStage {
    /// Assembling a reviewable plan; no system change may occur.
    PreparingPlan,
    /// The exact plan revision awaits a user decision.
    AwaitingApproval,
    /// The plan revision was approved.
    Approved,
    /// Transferring the installer artifact into GixGiz-owned staging.
    Downloading,
    /// Checking artifact identity, integrity, and publisher before any execution.
    VerifyingArtifact,
    /// Placing the verified artifact at its final staged location.
    Staging,
    /// Running the provider installer.
    Installing,
    /// Confirming the installed executable and its publisher.
    VerifyingExecutable,
    /// Starting the newly installed runtime when required.
    StartingRuntime,
    /// Confirming runtime version and endpoint health.
    VerifyingRuntime,
    /// Installation verified end to end.
    Ready,
    /// A newer peer supplied an unrecognized stage.
    #[serde(other)]
    Unknown,
}

/// Explicit reason an installation stopped and needs a user decision.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeInstallAttentionReason {
    /// The plan revision has no approval.
    ApprovalRequired,
    /// A runtime is already installed and is not owned by GixGiz.
    ExternalRuntimePresent,
    /// Artifact identity, integrity, or publisher evidence was rejected.
    ArtifactRejected,
    /// The installer finished but its effect could not be established.
    InstallOutcomeUncertain,
    /// The runtime could not be verified after installation.
    RuntimeUnverified,
    /// Not enough storage remains for the declared artifact.
    InsufficientStorage,
    /// A newer peer supplied an unrecognized reason.
    #[serde(other)]
    Unknown,
}

/// Safe classification of a runtime-installation failure.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeInstallFailureCode {
    /// The requested source is not an allowlisted trusted origin.
    SourceUntrusted,
    /// The artifact could not be transferred within its bounds.
    DownloadFailed,
    /// The artifact exceeded its declared or absolute size bound.
    ArtifactTooLarge,
    /// The artifact digest did not match trusted expected evidence.
    IntegrityMismatch,
    /// The artifact carried no valid signature.
    SignatureInvalid,
    /// The artifact was signed by an unexpected publisher.
    PublisherUnexpected,
    /// GixGiz-owned staging was unavailable or failed validation.
    StagingUnavailable,
    /// Not enough free space to transfer or install the runtime.
    InsufficientStorage,
    /// The provider installer reported an unsuccessful result.
    InstallerFailed,
    /// No installed executable was found after installation.
    ExecutableNotFound,
    /// The installed executable failed identity or publisher verification.
    ExecutableUntrusted,
    /// The installed runtime version is outside supported evidence.
    VersionUnsupported,
    /// The installed runtime did not become healthy.
    RuntimeUnhealthy,
    /// Ownership could not be established safely.
    OwnershipConflict,
    /// Durable state could not be read or written.
    PersistenceUnavailable,
    /// The operation exceeded its deadline.
    TimedOut,
    /// The operation was cancelled.
    Cancelled,
    /// A newer peer supplied an unrecognized failure.
    #[serde(other)]
    Unknown,
}

/// Safe recovery route offered after a stopped installation.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeInstallRecoveryAction {
    /// Retry the same approved plan revision.
    Retry,
    /// Discard this plan and prepare a new one.
    RecreatePlan,
    /// Use the already-present external runtime instead of installing.
    ReuseExternal,
    /// Free storage and retry.
    FreeStorage,
    /// No safe automatic route exists.
    NoAction,
    /// A newer peer supplied an unrecognized action.
    #[serde(other)]
    Unknown,
}

/// Category of material system effect an installation may produce.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeInstallEffectKind {
    /// An installer artifact placed in GixGiz-owned staging.
    StagedInstallerArtifact,
    /// Application files written outside the GixGiz data root.
    PerUserApplicationFiles,
    /// A per-user uninstall registration created by the provider installer.
    PerUserUninstallRegistration,
    /// A runtime process started by GixGiz.
    RuntimeProcessStarted,
    /// A GixGiz-owned ownership record.
    OwnershipRecorded,
    /// A newer peer supplied an unrecognized effect.
    #[serde(other)]
    Unknown,
}

/// What actually happened to one declared effect.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeInstallEffectDisposition {
    /// The effect was never started.
    NotStarted,
    /// The effect completed and remains in place.
    Completed,
    /// The effect was undone by GixGiz.
    RolledBack,
    /// The effect completed and was deliberately kept for recovery.
    Retained,
    /// Whether the effect applies cannot be established.
    Uncertain,
    /// A newer peer supplied an unrecognized disposition.
    #[serde(other)]
    Unknown,
}

/// One material effect and its observed disposition.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallEffect {
    /// Effect category.
    pub kind: RuntimeInstallEffectKind,
    /// Observed disposition; never assumed from an exit code.
    pub disposition: RuntimeInstallEffectDisposition,
    /// Bounded beginner-readable description containing no private paths.
    pub description: String,
}

/// Complete honest account of every declared effect.
#[derive(Clone, Debug, Default, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallEffectReport {
    /// Declared effects and what happened to each.
    pub effects: Vec<RuntimeInstallEffect>,
}

/// Where an installation writes.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeInstallDestinationCategory {
    /// A per-user application directory requiring no administrator rights.
    PerUserApplicationDirectory,
    /// A machine-wide location requiring administrator rights.
    MachineWideApplicationDirectory,
    /// A newer peer supplied an unrecognized destination.
    #[serde(other)]
    Unknown,
}

/// Category of integrity evidence available for an artifact.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeInstallIntegrityEvidence {
    /// A pinned cryptographic digest plus an expected signing publisher.
    DigestAndPublisher,
    /// An expected signing publisher only.
    PublisherOnly,
    /// No trustworthy evidence; installation must not proceed.
    None,
    /// A newer peer supplied an unrecognized evidence kind.
    #[serde(other)]
    Unknown,
}

/// One reviewable component of a runtime-installation plan.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallComponent {
    /// Beginner-readable component name.
    pub display_name: String,
    /// Exact version this plan installs.
    pub version: String,
    /// Bounded trusted origin shown to the user, without query parameters.
    pub source_origin: String,
    /// Bounded artifact file name.
    pub artifact_name: String,
    /// Conservative expected transfer size.
    pub expected_size_bytes: u64,
    /// Integrity evidence that will be enforced before any execution.
    pub integrity_evidence: RuntimeInstallIntegrityEvidence,
    /// Expected signing publisher shown to the user.
    pub expected_publisher: String,
}

/// Immutable reviewable description of one exact runtime installation.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallPlan {
    /// Runtime-installation schema generation.
    pub schema_version: u32,
    /// Durable job that owns this plan.
    pub job_id: RuntimeInstallJobId,
    /// Monotonic plan revision that approval is bound to.
    pub revision: u32,
    /// Runtime provider this plan installs.
    pub provider_id: RuntimeProviderId,
    /// Beginner-readable runtime name.
    pub runtime_display_name: RuntimeDisplayName,
    /// Components the user is approving.
    pub components: Vec<RuntimeInstallComponent>,
    /// Where the installation writes.
    pub destination: RuntimeInstallDestinationCategory,
    /// Total conservative expected download size.
    pub expected_download_bytes: u64,
    /// Whether Windows will require administrator approval.
    ///
    /// A per-user installation is `false`. This never implies GixGiz approval:
    /// the plan must be approved regardless of this value.
    pub requires_administrator: bool,
    /// Ownership recorded only after full verification succeeds.
    pub ownership_after_success: RuntimeOwnership,
    /// Material effects this approval authorizes and nothing more.
    pub authorized_effects: Vec<RuntimeInstallEffectKind>,
    /// Safe deterministic plan reasons.
    pub reasons: Vec<String>,
    /// Safe plan warnings and limitations, including what cannot be rolled back.
    pub warnings: Vec<String>,
}

/// Explicit decision for one exact runtime-installation plan revision.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeInstallApprovalDecision {
    /// Approve only this exact plan revision and its recorded effects.
    Approve,
    /// Deny this plan revision.
    Deny,
    /// A newer client supplied an unrecognized decision.
    #[serde(other)]
    Unknown,
}

/// Durable approval record created by Rust after validating the plan revision.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallApprovalRecord {
    /// Approved durable job.
    pub job_id: RuntimeInstallJobId,
    /// Approved plan revision; a different revision requires a new approval.
    pub plan_revision: u32,
    /// Explicit user decision.
    pub decision: RuntimeInstallApprovalDecision,
    /// Approved runtime provider.
    pub provider_id: RuntimeProviderId,
    /// Approved exact version.
    pub version: String,
    /// Approved material effects.
    pub authorized_effects: Vec<RuntimeInstallEffectKind>,
    /// Whether the approved plan expected an administrator prompt.
    pub requires_administrator: bool,
    /// Correlation identifier from the approval request.
    pub correlation_id: CorrelationId,
    /// Identifier of the approval request.
    pub request_id: RequestId,
    /// UTC Unix timestamp in milliseconds assigned by Rust.
    pub decided_at_unix_ms: u64,
}

/// Bounded transfer progress for the installer artifact.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallProgress {
    /// Bytes transferred into GixGiz-owned staging so far.
    pub transferred_bytes: u64,
    /// Conservative expected total when known.
    pub expected_bytes: Option<u64>,
}

/// Independent post-installation verification evidence.
///
/// Every field must be true before an installation may report success. A value
/// is only ever set from observed evidence, never from an installer exit code.
#[derive(Clone, Copy, Debug, Default, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallVerificationResult {
    /// An executable was found at an approved installation location.
    pub executable_located: bool,
    /// The executable carried a valid signature.
    pub signature_valid: bool,
    /// The signing publisher matched the approved expectation.
    pub publisher_matched: bool,
    /// The reported version is within supported evidence.
    pub version_supported: bool,
    /// The approved loopback endpoint reported health.
    pub endpoint_healthy: bool,
}

impl RuntimeInstallVerificationResult {
    /// Returns whether every independent check passed.
    #[must_use]
    pub const fn is_verified(self) -> bool {
        self.executable_located
            && self.signature_valid
            && self.publisher_matched
            && self.version_supported
            && self.endpoint_healthy
    }
}

/// Authoritative persistent runtime-installation job snapshot.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallJobSnapshot {
    /// Runtime-installation schema generation.
    pub schema_version: u32,
    /// Durable job identifier.
    pub job_id: RuntimeInstallJobId,
    /// Current durable state.
    pub state: RuntimeInstallState,
    /// Furthest stage actually reached.
    pub stage: RuntimeInstallStage,
    /// Current immutable plan.
    pub plan: RuntimeInstallPlan,
    /// Approval when one exists for the current revision.
    pub approval: Option<RuntimeInstallApprovalRecord>,
    /// Bounded transfer progress when a transfer started.
    pub progress: Option<RuntimeInstallProgress>,
    /// Verification evidence when verification ran.
    pub verification: Option<RuntimeInstallVerificationResult>,
    /// Honest account of every declared effect.
    pub effects: RuntimeInstallEffectReport,
    /// Reason the job needs a decision, when it does.
    pub attention: Option<RuntimeInstallAttentionReason>,
    /// Safe failure classification when the job failed.
    pub failure: Option<RuntimeInstallFailureCode>,
    /// Safe recovery route when one exists.
    pub recovery: Option<RuntimeInstallRecoveryAction>,
    /// Ownership currently recorded for the provider.
    pub ownership: RuntimeOwnership,
    /// Number of retries already consumed.
    pub retry_count: u32,
    /// UTC Unix timestamp in milliseconds of the last durable transition.
    pub updated_at_unix_ms: u64,
}

/// Streamed runtime-installation event kind.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeInstallEventKind {
    /// The job entered a new stage.
    StageChanged,
    /// Bounded transfer progress advanced.
    Progress,
    /// The job reached a terminal or attention state.
    Terminal,
    /// A newer peer supplied an unrecognized event.
    #[serde(other)]
    Unknown,
}

/// One ordered runtime-installation event.
///
/// Events are a convenience for live observation. Durable SQLite state remains
/// authoritative, and a terminal event is published only after its transition
/// commits.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallEvent {
    /// Runtime-installation schema generation.
    pub schema_version: u32,
    /// Job this event belongs to.
    pub job_id: RuntimeInstallJobId,
    /// Monotonic sequence within the job.
    pub sequence: u64,
    /// Event category.
    pub kind: RuntimeInstallEventKind,
    /// Stage at the time of the event.
    pub stage: RuntimeInstallStage,
    /// Durable state at the time of the event.
    pub state: RuntimeInstallState,
    /// Bounded transfer progress when relevant.
    pub progress: Option<RuntimeInstallProgress>,
    /// Safe failure classification on a failed terminal event.
    pub failure: Option<RuntimeInstallFailureCode>,
    /// Correlation identifier shared by the whole job.
    pub correlation_id: CorrelationId,
    /// UTC Unix timestamp in milliseconds.
    pub occurred_at_unix_ms: u64,
}

/// Requests a reviewable installation plan; performs no system change.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallPlanRequest {
    /// Provider to plan an installation for.
    pub provider_id: RuntimeProviderId,
    /// Identifier shared with the response and any failure.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Reviewable plan, or an explicit reason no plan is offered.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallPlanResponse {
    /// Plan when an installation is offered.
    pub plan: Option<RuntimeInstallPlan>,
    /// Reason no plan is offered, such as an existing external runtime.
    pub attention: Option<RuntimeInstallAttentionReason>,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Records one explicit decision for one exact plan revision.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallApprovalRequest {
    /// Job whose plan is being decided.
    pub job_id: RuntimeInstallJobId,
    /// Exact plan revision the user reviewed.
    pub plan_revision: u32,
    /// Explicit decision.
    pub decision: RuntimeInstallApprovalDecision,
    /// Identifier shared with the response and any failure.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Authoritative job state after the approval write.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallApprovalResponse {
    /// Authoritative snapshot after the decision.
    pub job: RuntimeInstallJobSnapshot,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Starts approved installation work.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallStartRequest {
    /// Approved job to start.
    pub job_id: RuntimeInstallJobId,
    /// Identifier shared with every event and failure.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Accepted installation start.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallStartResponse {
    /// Authoritative snapshot after admission.
    pub job: RuntimeInstallJobSnapshot,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Reads authoritative durable job state.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallStatusRequest {
    /// Job to read.
    pub job_id: RuntimeInstallJobId,
    /// Identifier shared with the response.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Authoritative durable job state.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallStatusResponse {
    /// Snapshot when the job exists.
    pub job: Option<RuntimeInstallJobSnapshot>,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Requests cancellation of in-progress installation work.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallCancelRequest {
    /// Job to cancel.
    pub job_id: RuntimeInstallJobId,
    /// Identifier shared with the response.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Authoritative state after a cancellation request.
///
/// Cancellation is a request, not an assertion: an installer already running
/// may not stop at an arbitrary point, and effects are reported honestly.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallCancelResponse {
    /// Authoritative snapshot after the cancellation attempt.
    pub job: RuntimeInstallJobSnapshot,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Reconciles a job interrupted by an earlier process exit.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallRecoveryRequest {
    /// Identifier shared with the response.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Authoritative state after honest reconciliation through detection.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallRecoveryResponse {
    /// Reconciled job when one was interrupted.
    pub job: Option<RuntimeInstallJobSnapshot>,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Replays a bounded page of ordered job events.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallEventsRequest {
    /// Job whose events are replayed.
    pub job_id: RuntimeInstallJobId,
    /// Exclusive lower bound on sequence.
    pub after_sequence: Option<u64>,
    /// Maximum events to return, bounded by `MAX_INSTALL_EVENT_PAGE`.
    pub limit: u16,
    /// Identifier shared with the response.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Bounded ordered event page.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallEventsResponse {
    /// Ordered events.
    pub events: Vec<RuntimeInstallEvent>,
    /// Whether more events exist after this page.
    pub truncated: bool,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Safe failure payload for the runtime-installation boundary.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInstallFailure {
    /// Safe transport-level payload.
    pub error: SafeErrorPayload,
    /// Installation-specific classification when available.
    pub code: Option<RuntimeInstallFailureCode>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_verified_evidence_reports_success() {
        let mut result = RuntimeInstallVerificationResult {
            executable_located: true,
            signature_valid: true,
            publisher_matched: true,
            version_supported: true,
            endpoint_healthy: true,
        };
        assert!(result.is_verified());

        // Dropping any single check must withdraw verification entirely.
        for clear in [
            |r: &mut RuntimeInstallVerificationResult| r.executable_located = false,
            |r: &mut RuntimeInstallVerificationResult| r.signature_valid = false,
            |r: &mut RuntimeInstallVerificationResult| r.publisher_matched = false,
            |r: &mut RuntimeInstallVerificationResult| r.version_supported = false,
            |r: &mut RuntimeInstallVerificationResult| r.endpoint_healthy = false,
        ] {
            let mut candidate = result;
            clear(&mut candidate);
            assert!(!candidate.is_verified());
        }

        result.endpoint_healthy = false;
        assert!(!result.is_verified());
    }

    #[test]
    fn default_verification_claims_nothing() {
        assert!(!RuntimeInstallVerificationResult::default().is_verified());
    }

    #[test]
    fn only_finished_states_are_terminal() {
        for state in [
            RuntimeInstallState::Ready,
            RuntimeInstallState::Failed,
            RuntimeInstallState::Cancelled,
        ] {
            assert!(state.is_terminal());
        }
        for state in [
            RuntimeInstallState::AwaitingApproval,
            RuntimeInstallState::Approved,
            RuntimeInstallState::Running,
            RuntimeInstallState::AttentionRequired,
        ] {
            assert!(!state.is_terminal());
        }
    }

    #[test]
    fn unknown_peer_values_deserialize_without_failing_closed_types() {
        let state: RuntimeInstallState =
            serde_json::from_str("\"a_future_state\"").expect("state tolerates newer peers");
        assert_eq!(state, RuntimeInstallState::Unknown);

        let stage: RuntimeInstallStage =
            serde_json::from_str("\"a_future_stage\"").expect("stage tolerates newer peers");
        assert_eq!(stage, RuntimeInstallStage::Unknown);

        let code: RuntimeInstallFailureCode =
            serde_json::from_str("\"a_future_code\"").expect("code tolerates newer peers");
        assert_eq!(code, RuntimeInstallFailureCode::Unknown);
    }

    #[test]
    fn stages_keep_their_reviewable_order() {
        assert!(RuntimeInstallStage::PreparingPlan < RuntimeInstallStage::AwaitingApproval);
        assert!(RuntimeInstallStage::AwaitingApproval < RuntimeInstallStage::Approved);
        // Verification always follows transfer and precedes any execution.
        assert!(RuntimeInstallStage::Downloading < RuntimeInstallStage::VerifyingArtifact);
        assert!(RuntimeInstallStage::VerifyingArtifact < RuntimeInstallStage::Installing);
        assert!(RuntimeInstallStage::Installing < RuntimeInstallStage::VerifyingExecutable);
        assert!(RuntimeInstallStage::VerifyingRuntime < RuntimeInstallStage::Ready);
    }
}
