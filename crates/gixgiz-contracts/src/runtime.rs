use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{CandidateModelId, CorrelationId, OperationId, RequestId, SafeErrorPayload};

/// Schema generation for provider-neutral runtime reports and inventories.
pub const RUNTIME_REPORT_SCHEMA_VERSION: u32 = 1;

macro_rules! runtime_string_type {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(
            Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Creates a value from adapter-normalized bounded text.
            #[must_use]
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            /// Returns the normalized text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

runtime_string_type!(
    RuntimeProviderId,
    "Stable provider-neutral runtime provider identifier."
);
runtime_string_type!(
    RuntimeDisplayName,
    "Bounded display name for a runtime provider."
);
runtime_string_type!(
    RuntimeProviderModelId,
    "Opaque provider model identifier normalized by a runtime adapter."
);

/// Normalized lifecycle and health state for a local runtime.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeState {
    /// No verified installation or reachable provider was found.
    NotInstalled,
    /// A verified installation exists but its local endpoint is stopped.
    InstalledStopped,
    /// A bounded core-owned start operation is in progress.
    Starting,
    /// Required local health and compatibility evidence is verified.
    Ready,
    /// The provider is reachable but some required evidence is incomplete.
    Degraded,
    /// Required provider capabilities or version policy are incompatible.
    Incompatible,
    /// A bounded update operation is in progress.
    Updating,
    /// A terminal lifecycle operation failed.
    Failed,
    /// A newer peer supplied an unrecognized state.
    #[serde(other)]
    Unknown,
}

/// Management ownership recorded independently from provider detection.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeOwnership {
    /// The installation is controlled outside GixGiz.
    External,
    /// GixGiz may manage the installation under recorded policy.
    GixGizManaged,
    /// The runtime is distributed as a controlled GixGiz component.
    Bundled,
    /// Ownership has not been established or was supplied by a newer peer.
    #[serde(other)]
    Unknown,
}

/// Recorded decision for one runtime consent scope.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeConsentState {
    /// The user has not made a decision for this scope.
    NotRequested,
    /// Read-only reuse of a compatible external runtime is approved.
    ReuseApproved,
    /// Lifecycle management is explicitly approved.
    ManagementApproved,
    /// The user denied the requested scope.
    Denied,
    /// The consent state is unavailable or was supplied by a newer peer.
    #[serde(other)]
    Unknown,
}

/// User decision accepted by the Task 09 reuse-consent boundary.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeConsentDecision {
    /// Permit read-only reuse without changing ownership.
    ApproveReuse,
    /// Refuse read-only reuse.
    DenyReuse,
    /// Accept one exact runtime version that is outside recorded support evidence.
    ///
    /// The acknowledgement is bound to that exact version. A later provider
    /// version is untested again and requires a new explicit decision.
    AcknowledgeUntestedVersion,
    /// Withdraw a previous untested-version acknowledgement.
    RevokeUntestedVersion,
    /// A newer client supplied an unrecognized decision.
    #[serde(other)]
    Unknown,
}

/// Safety classification for the selected provider endpoint.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEndpointSafety {
    /// The adapter verified a fixed local loopback endpoint before I/O.
    LoopbackVerified,
    /// The configured endpoint is not restricted to loopback.
    Unsafe,
    /// Endpoint safety could not be established.
    Unverified,
    /// A newer peer supplied an unrecognized safety classification.
    #[serde(other)]
    Unknown,
}

/// Compatibility result for normalized provider version evidence.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeVersionCompatibility {
    /// Required provider capabilities and version syntax were verified.
    Compatible,
    /// Required capabilities are known not to match this adapter.
    Incompatible,
    /// The version is valid but outside recorded real-provider evidence.
    Untested,
    /// Compatibility is unavailable or was supplied by a newer peer.
    #[serde(other)]
    Unknown,
}

/// Safe normalized provider version information.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeVersionInfo {
    /// Bounded version text returned by the provider.
    pub reported_version: String,
    /// Semantic version normalized by the adapter when parsing succeeds.
    pub normalized_version: Option<String>,
    /// Compatibility derived from required capability and version evidence.
    pub compatibility: RuntimeVersionCompatibility,
}

/// Provider-neutral runtime capability category.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCapabilityKind {
    /// Detect installation and endpoint evidence.
    Detection,
    /// Query normalized provider health.
    Health,
    /// Query provider version evidence.
    Version,
    /// Start a core-managed provider process.
    Start,
    /// Stop the exact process owned by the active adapter session.
    Stop,
    /// Perform one bounded stop followed by one start.
    Restart,
    /// List a bounded normalized installed-model inventory.
    ModelInventory,
    /// Resolve one canonical model to an approved provider artifact.
    ModelAcquisitionPreparation,
    /// Check provider-managed storage before model acquisition.
    ModelStoragePreflight,
    /// Acquire one exact approved provider artifact with normalized progress.
    ModelAcquisition,
    /// Inspect exact local model availability and provider registration.
    ModelRegistration,
    /// Run one fixed bounded readiness inference without returning generated content.
    ReadinessInference,
    /// A newer peer supplied an unrecognized capability.
    #[serde(other)]
    Unknown,
}

/// Current policy availability for one runtime capability.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCapabilityAvailability {
    /// The capability is currently available.
    Available,
    /// The capability requires explicit external-runtime reuse consent.
    RequiresReuseConsent,
    /// The capability requires explicit management ownership and consent.
    RequiresManagementConsent,
    /// The capability requires approval of one exact durable setup plan.
    RequiresSetupApproval,
    /// The provider or current ownership cannot safely perform the capability.
    Unsupported,
    /// Availability is unknown or was supplied by a newer peer.
    #[serde(other)]
    Unknown,
}

/// Core-owned action availability presented without duplicating policy in Flutter.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeCapabilityDescriptor {
    /// Capability being described.
    pub kind: RuntimeCapabilityKind,
    /// Current provider and policy availability.
    pub availability: RuntimeCapabilityAvailability,
    /// Optional bounded explanation safe for UI presentation.
    pub reason: Option<String>,
}

/// Stable explanation category for runtime state evidence.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeReasonCode {
    /// No validated installation evidence was found.
    InstallationNotFound,
    /// A validated executable was found in an approved location.
    ExecutableVerified,
    /// The approved local endpoint could not be reached.
    EndpointUnavailable,
    /// The approved local endpoint returned valid evidence.
    EndpointReachable,
    /// The endpoint failed local-only safety policy.
    EndpointUnsafe,
    /// Provider version and required capabilities are compatible.
    VersionCompatible,
    /// Required provider capabilities are incompatible.
    VersionIncompatible,
    /// Provider version compatibility could not be verified.
    VersionUnverified,
    /// Some provider evidence was valid but incomplete.
    EvidenceIncomplete,
    /// Current ownership does not permit the requested operation.
    OwnershipRequired,
    /// Explicit consent is required before the requested operation.
    ConsentRequired,
    /// The provider does not support the requested operation safely.
    OperationUnsupported,
    /// A managed provider process exited before becoming healthy.
    ProcessExited,
    /// A newer peer supplied an unrecognized reason.
    #[serde(other)]
    Unknown,
}

/// Safe bounded explanation attached to a runtime report.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeReason {
    /// Stable reason category.
    pub code: RuntimeReasonCode,
    /// Provider-neutral explanation with no raw provider output.
    pub message: String,
}

/// Stable warning category for runtime evidence or policy.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeWarningCode {
    /// The detected installation remains externally managed.
    ExternalInstallation,
    /// Provider configuration may expose the endpoint beyond loopback.
    EndpointExposure,
    /// The provider version lacks recorded real-provider evidence.
    VersionUntested,
    /// Some provider evidence remains explicitly incomplete.
    PartialEvidence,
    /// The returned model inventory was truncated to a safe bound.
    ModelInventoryTruncated,
    /// A newer peer supplied an unrecognized warning.
    #[serde(other)]
    Unknown,
}

/// Safe bounded runtime warning.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeWarning {
    /// Stable warning category.
    pub code: RuntimeWarningCode,
    /// Provider-neutral warning with no raw provider output.
    pub message: String,
}

/// Stable runtime-specific error code used before broad error categorization.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeErrorCode {
    /// The requested provider, operation, or bound is invalid.
    InvalidRequest,
    /// No validated runtime installation is available.
    NotInstalled,
    /// A discovered runtime executable failed the platform identity policy.
    ExecutableUntrusted,
    /// The endpoint violates local-only safety policy.
    EndpointUnsafe,
    /// The provider is currently unavailable.
    ProviderUnavailable,
    /// The provider returned invalid or excessive data.
    ProviderResponseInvalid,
    /// Required provider capabilities are incompatible.
    VersionIncompatible,
    /// The requested operation is not supported safely.
    OperationUnsupported,
    /// Explicit consent is required.
    ConsentRequired,
    /// Current ownership conflicts with the requested operation.
    OwnershipConflict,
    /// Another lifecycle operation is already active.
    OperationBusy,
    /// A managed provider process failed.
    ProcessFailed,
    /// The canonical model has no safe provider mapping.
    ModelNotMapped,
    /// Provider model acquisition failed safely.
    ModelAcquisitionFailed,
    /// The exact provider model is unavailable.
    ModelUnavailable,
    /// Provider registration could not be verified.
    ModelRegistrationFailed,
    /// Available artifact-integrity evidence did not match.
    ModelIntegrityFailed,
    /// The selected model destination is unavailable.
    ModelStorageUnavailable,
    /// The selected destination lacks the required storage margin.
    ModelStorageExhausted,
    /// The fixed bounded readiness inference failed.
    ReadinessInferenceFailed,
    /// Provider chat generation ended without a validated terminal completion.
    GenerationFailed,
    /// Provider output exceeded a fixed bound.
    OutputLimit,
    /// Explicit cancellation was observed.
    Cancelled,
    /// The operation exceeded its deadline.
    TimedOut,
    /// The failure has no safer stable category.
    Internal,
    /// A newer peer supplied an unrecognized code.
    #[serde(other)]
    Unknown,
}

/// Authoritative provider-neutral runtime status.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeHealthReport {
    /// Runtime report schema generation.
    pub schema_version: u32,
    /// Stable runtime provider identity.
    pub provider_id: RuntimeProviderId,
    /// User-facing provider display name.
    pub display_name: RuntimeDisplayName,
    /// Normalized runtime state.
    pub state: RuntimeState,
    /// Ownership recorded independently from detection.
    pub ownership: RuntimeOwnership,
    /// Consent for read-only external-runtime reuse.
    pub reuse_consent: RuntimeConsentState,
    /// Consent for process lifecycle management.
    pub management_consent: RuntimeConsentState,
    /// Safety of the selected provider endpoint.
    pub endpoint_safety: RuntimeEndpointSafety,
    /// Provider version evidence when available.
    pub version: Option<RuntimeVersionInfo>,
    /// Exact provider version the user accepted while it remains untested.
    ///
    /// `None` means no acknowledgement applies to the detected version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acknowledged_untested_version: Option<String>,
    /// Core-owned current action availability.
    pub capabilities: Vec<RuntimeCapabilityDescriptor>,
    /// Safe reasons supporting the state.
    pub reasons: Vec<RuntimeReason>,
    /// Safe non-terminal warnings.
    pub warnings: Vec<RuntimeWarning>,
}

/// Confidence of a provider-model to catalogue-model mapping.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeModelMappingStatus {
    /// The adapter recognized an exact provider identifier mapping.
    Matched,
    /// The provider model remains external to the current catalogue.
    External,
    /// Mapping is unavailable or was supplied by a newer peer.
    #[serde(other)]
    Unknown,
}

/// Provider-neutral mapping from one provider model to the local catalogue.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeProviderModelMapping {
    /// Mapping confidence category.
    pub status: RuntimeModelMappingStatus,
    /// Stable catalogue identity only for an exact known mapping.
    pub catalogue_id: Option<CandidateModelId>,
}

/// One bounded provider model record with no digest or raw payload.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeModelSummary {
    /// Opaque adapter-normalized provider model identifier.
    pub provider_model_id: RuntimeProviderModelId,
    /// Bounded display name.
    pub display_name: String,
    /// Provider-reported size in bytes when valid.
    pub size_bytes: Option<u64>,
    /// Optional exact mapping to the local planning catalogue.
    pub mapping: RuntimeProviderModelMapping,
}

/// Bounded installed-model inventory returned only on explicit request.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeModelInventory {
    /// Runtime report schema generation.
    pub schema_version: u32,
    /// Provider that supplied the inventory.
    pub provider_id: RuntimeProviderId,
    /// Normalized model records in stable provider order.
    pub models: Vec<RuntimeModelSummary>,
    /// Whether additional provider records were omitted by a safe bound.
    pub truncated: bool,
    /// UTC Unix timestamp in milliseconds captured after collection.
    pub collected_at_unix_ms: u64,
}

/// Authenticated request for current runtime status.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeStatusRequest {
    /// Requested provider identity.
    pub provider_id: RuntimeProviderId,
    /// Identifier shared with the response and any failure.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Current authoritative runtime status with boundary identifiers.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeStatusResponse {
    /// Core-composed runtime report.
    pub report: RuntimeHealthReport,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Authenticated decision for external-runtime reuse.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeConsentRequest {
    /// Provider to which the decision applies.
    pub provider_id: RuntimeProviderId,
    /// Explicit reuse decision; management approval is not granted here.
    pub decision: RuntimeConsentDecision,
    /// Exact normalized version acknowledged by `AcknowledgeUntestedVersion`.
    ///
    /// Required for that decision and ignored otherwise. Rust rejects the
    /// request when this value does not match currently detected evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acknowledged_version: Option<String>,
    /// Identifier shared with the response and any failure.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Runtime report after applying an explicit reuse decision.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeConsentResponse {
    /// Authoritative report after the policy write.
    pub report: RuntimeHealthReport,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Authenticated bounded installed-model inventory request.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeModelInventoryRequest {
    /// Requested provider identity.
    pub provider_id: RuntimeProviderId,
    /// Maximum number of records requested; the core applies a lower safe cap.
    pub limit: u16,
    /// Identifier shared with the response and any failure.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Bounded normalized inventory with boundary identifiers.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeModelInventoryResponse {
    /// Normalized provider inventory.
    pub inventory: RuntimeModelInventory,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Provider-neutral lifecycle operation.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeOperationKind {
    /// Start a managed provider process.
    Start,
    /// Stop the exact provider process owned by this adapter session.
    Stop,
    /// Perform one bounded stop followed by one start.
    Restart,
    /// A newer peer supplied an unrecognized operation.
    #[serde(other)]
    Unknown,
}

/// Authenticated request to start one bounded lifecycle operation.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeOperationStartRequest {
    /// Provider that should perform the operation.
    pub provider_id: RuntimeProviderId,
    /// Requested lifecycle operation.
    pub kind: RuntimeOperationKind,
    /// Identifier shared by all operation events and failures.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this start request.
    pub request_id: RequestId,
}

/// Accepted runtime lifecycle operation.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeOperationStartResponse {
    /// Opaque identifier used for events and cancellation.
    pub operation_id: OperationId,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Stable runtime lifecycle event kind.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeOperationEventKind {
    /// The operation was accepted.
    Started,
    /// Bounded lifecycle work is in progress.
    Progress,
    /// The lifecycle operation completed.
    Completed,
    /// Explicit cancellation was observed.
    Cancelled,
    /// The lifecycle operation failed safely.
    Failed,
    /// The operation exceeded its deadline.
    TimedOut,
    /// A newer peer supplied an unrecognized event kind.
    #[serde(other)]
    Unknown,
}

/// Terminal result for one runtime lifecycle operation.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum RuntimeOperationTerminalState {
    /// The requested effect completed and was re-observed.
    Completed,
    /// Explicit cancellation was observed.
    Cancelled,
    /// The operation failed safely; partial effects are described by the report.
    Failed,
    /// The operation exceeded its deadline.
    TimedOut,
    /// A newer peer supplied an unrecognized terminal state.
    #[serde(other)]
    Unknown,
}

/// Ordered provider-neutral runtime lifecycle event.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RuntimeOperationEvent {
    /// Runtime report schema generation.
    pub schema_version: u32,
    /// Operation that owns this stream.
    pub operation_id: OperationId,
    /// Identifier shared by every event for the operation.
    pub correlation_id: CorrelationId,
    /// Monotonically increasing sequence number starting at one.
    pub sequence: u64,
    /// Requested lifecycle operation.
    pub operation_kind: RuntimeOperationKind,
    /// Stable event classification.
    pub kind: RuntimeOperationEventKind,
    /// UTC Unix timestamp in milliseconds captured by the host.
    pub timestamp_unix_ms: u64,
    /// Optional safe provider-neutral progress explanation.
    pub message: Option<String>,
    /// Authoritative report when new status evidence is available.
    pub report: Option<RuntimeHealthReport>,
    /// Safe typed failure for failed or timed-out terminal events.
    pub error: Option<SafeErrorPayload>,
    /// Terminal state present only on the final event.
    pub terminal_state: Option<RuntimeOperationTerminalState>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_runtime_states_and_policy_values_are_stable() {
        assert_eq!(RUNTIME_REPORT_SCHEMA_VERSION, 1);
        assert_eq!(
            serde_json::to_string(&RuntimeState::InstalledStopped).expect("state serializes"),
            "\"installed_stopped\""
        );
        assert_eq!(
            serde_json::to_string(&RuntimeOwnership::GixGizManaged).expect("ownership serializes"),
            "\"gix_giz_managed\""
        );
        assert_eq!(
            serde_json::to_string(&RuntimeConsentState::ManagementApproved)
                .expect("consent serializes"),
            "\"management_approved\""
        );
        assert_eq!(
            serde_json::to_string(&RuntimeErrorCode::ExecutableUntrusted)
                .expect("runtime error code serializes"),
            "\"executable_untrusted\""
        );

        let all_states = [
            RuntimeState::NotInstalled,
            RuntimeState::InstalledStopped,
            RuntimeState::Starting,
            RuntimeState::Ready,
            RuntimeState::Degraded,
            RuntimeState::Incompatible,
            RuntimeState::Updating,
            RuntimeState::Failed,
        ];
        assert_eq!(all_states.len(), 8);
    }

    #[test]
    fn external_enums_accept_future_values_without_becoming_ready() {
        let state: RuntimeState =
            serde_json::from_str("\"future_state\"").expect("future state accepted");
        let ownership: RuntimeOwnership =
            serde_json::from_str("\"future_owner\"").expect("future owner accepted");
        let error: RuntimeErrorCode =
            serde_json::from_str("\"future_error\"").expect("future error accepted");

        assert_eq!(state, RuntimeState::Unknown);
        assert_eq!(ownership, RuntimeOwnership::Unknown);
        assert_eq!(error, RuntimeErrorCode::Unknown);
    }

    #[test]
    fn shared_runtime_schema_contains_no_provider_implementation_details() {
        let schema = serde_json::to_string(&schemars::schema_for!(RuntimeHealthReport))
            .expect("runtime schema serializes")
            .to_ascii_lowercase();

        for forbidden in [
            "ollama",
            "/api/",
            "serve",
            "executable_path",
            "command_output",
            "raw_payload",
            "digest",
        ] {
            assert!(!schema.contains(forbidden), "schema contains {forbidden}");
        }
    }
}
