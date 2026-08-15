use gixgiz_contracts::{
    CandidateModelId, ModelAcquisitionProgress, ModelIntegrityState, ModelProviderArtifact,
    ProviderRegistrationResult, SetupDestinationCategory,
};
use tokio::sync::mpsc;

use crate::{RuntimeFuture, RuntimeOperationContext};

/// Recommended bounded capacity for normalized model-acquisition progress.
///
/// Providers may coalesce or discard intermediate updates when this channel is full. Persistent
/// job state remains authoritative, and the terminal method result is never carried only here.
pub const MODEL_PROGRESS_CHANNEL_CAPACITY: usize = 16;

/// Bounded sender used for provider-neutral model-acquisition progress.
pub type ModelProgressSender = mpsc::Sender<ModelAcquisitionProgress>;

/// Provider behavior after cooperative cancellation interrupts an acquisition request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RuntimeCancellationSemantics {
    /// The provider confirms that it stops acquisition without retaining effects.
    Cooperative,
    /// Cancelling closes the active request, while provider-managed partial or completed data may
    /// remain and must be reported by a later exact inspection.
    ConnectionAbortMayRetainEffects,
    /// The provider cannot safely expose cancellation for this operation.
    Unsupported,
}

/// Provider-neutral storage-preflight conclusion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RuntimeStorageAvailability {
    /// Available space satisfies the required bytes and safety margin.
    Available,
    /// Available space is known and does not satisfy the requirement.
    InsufficientSpace,
    /// The provider-managed destination is currently unavailable.
    Unavailable,
    /// The adapter could not establish bounded trustworthy space evidence.
    Unverified,
}

/// Acquisition terminal status for one exact artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RuntimeModelAcquisitionStatus {
    /// Exact provider registration existed before acquisition and no transfer was needed.
    AlreadyPresent,
    /// Provider acquisition reached a validated successful terminal response.
    Acquired,
}

/// Exact provider mapping and safe destination evidence prepared before approval.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeModelAcquisitionPlan {
    /// Canonical identity mapped to one opaque provider artifact.
    pub artifact: ModelProviderArtifact,
    /// Provider-controlled destination category.
    pub destination: SetupDestinationCategory,
    /// Bounded destination label that contains no private local path.
    pub destination_display: String,
    /// Provider-specific cancellation limitations expressed without provider details.
    pub cancellation: RuntimeCancellationSemantics,
}

/// Bounded provider-managed storage evidence captured immediately before acquisition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeStoragePreflight {
    /// Safe destination category from the prepared plan.
    pub destination: SetupDestinationCategory,
    /// Bounded destination label that contains no private local path.
    pub destination_display: String,
    /// Normalized availability conclusion.
    pub availability: RuntimeStorageAvailability,
    /// Exact conservative model requirement supplied by core policy.
    pub required_bytes: u64,
    /// Additional free-space margin supplied by core policy.
    pub safety_margin_bytes: u64,
    /// Available bytes when trustworthy bounded evidence was obtained.
    pub available_bytes: Option<u64>,
    /// UTC Unix timestamp in milliseconds captured after the check.
    pub checked_at_unix_ms: u64,
}

/// Successful terminal acquisition evidence for one exact provider artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeModelAcquisitionResult {
    /// Exact canonical/provider mapping that was acquired or already present.
    pub artifact: ModelProviderArtifact,
    /// Whether acquisition transferred data or reused exact registration.
    pub status: RuntimeModelAcquisitionStatus,
    /// Provider-reported installed size when valid.
    pub measured_size_bytes: Option<u64>,
    /// Available normalized artifact-integrity evidence.
    pub integrity: ModelIntegrityState,
    /// UTC Unix timestamp in milliseconds captured after terminal success.
    pub completed_at_unix_ms: u64,
}

/// Exact local model availability and provider-registration evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeModelInspection {
    /// Artifact that was inspected without provider-side mutation.
    pub artifact: ModelProviderArtifact,
    /// Whether the exact artifact is locally available for inference.
    pub available: bool,
    /// Bounded provider-registration evidence.
    pub registration: ProviderRegistrationResult,
    /// Available normalized artifact-integrity evidence.
    pub integrity: ModelIntegrityState,
}

/// Result of a fixed bounded readiness inference with generated content discarded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeReadinessInferenceResult {
    /// Whether the provider produced a valid non-empty response and successful terminal signal.
    pub ready: bool,
    /// UTC Unix timestamp in milliseconds captured after the readiness gate.
    pub completed_at_unix_ms: u64,
}

/// Provider-neutral model preparation and readiness operations consumed by setup orchestration.
pub trait RuntimeModelSetupProvider: Send + Sync {
    /// Maps one canonical catalogue identity to an exact safe provider artifact and destination.
    fn prepare_model_acquisition(
        &self,
        model_id: CandidateModelId,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeModelAcquisitionPlan>;

    /// Checks provider-managed storage for the exact core-supplied requirement and margin.
    fn preflight_model_storage(
        &self,
        plan: RuntimeModelAcquisitionPlan,
        required_bytes: u64,
        safety_margin_bytes: u64,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeStoragePreflight>;

    /// Acquires one exact approved artifact and reports bounded normalized progress.
    ///
    /// The caller must consume `progress` concurrently. Providers must not expose raw provider
    /// text through it and must not rely on it as the sole terminal result.
    fn acquire_model(
        &self,
        plan: RuntimeModelAcquisitionPlan,
        progress: ModelProgressSender,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeModelAcquisitionResult>;

    /// Inspects exact local availability and registration without mutation.
    fn inspect_model(
        &self,
        artifact: ModelProviderArtifact,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeModelInspection>;

    /// Runs one adapter-owned fixed readiness inference and discards generated content.
    fn run_readiness_inference(
        &self,
        artifact: ModelProviderArtifact,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeReadinessInferenceResult>;
}
