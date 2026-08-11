use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    ApplicationInfo, CorrelationId, PlatformStatus, ReadinessReport, RequestId, RuntimeProviderId,
    SCHEMA_VERSION,
};

/// Capabilities negotiated between the desktop and the internal core sidecar.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum TransportCapability {
    /// Query core identity, health, and readiness.
    Health,
    /// Start and observe the deterministic Task 04 event operation.
    TestOperationEvents,
    /// Explicitly cancel an operation.
    Cancellation,
    /// Start and observe a bounded hardware evidence scan.
    HardwareScan,
    /// Generate a deterministic capability recommendation from supplied evidence.
    CapabilityRecommendation,
    /// Query normalized local runtime status.
    RuntimeStatus,
    /// Record explicit external-runtime reuse consent.
    RuntimeConsent,
    /// Start, observe, and cancel bounded runtime lifecycle operations.
    RuntimeLifecycle,
    /// List a bounded normalized installed-model inventory.
    RuntimeModelInventory,
    /// Request bounded sidecar shutdown.
    Shutdown,
    /// A newer peer supplied a capability this build does not recognize.
    #[serde(other)]
    Unknown,
}

/// First authenticated request sent by the Flutter desktop.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ClientHello {
    /// Stable client name, such as the GixGiz desktop shell.
    pub client_name: String,
    /// Semantic version of the desktop client.
    pub client_version: String,
    /// Oldest protocol version accepted by the client.
    pub protocol_min: u32,
    /// Newest protocol version accepted by the client.
    pub protocol_max: u32,
    /// Optional capabilities requested for this launch session.
    pub requested_capabilities: Vec<TransportCapability>,
    /// Identifier shared by the handshake response and any safe failure.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this handshake request.
    pub request_id: RequestId,
}

/// Successful protocol and capability negotiation returned by the core.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct CoreHello {
    /// Identity and versions compiled into the running core.
    pub application: ApplicationInfo,
    /// Protocol version selected from the overlapping supported ranges.
    pub selected_protocol: u32,
    /// Capabilities supported by this internal core build.
    pub supported_capabilities: Vec<TransportCapability>,
    /// Opaque identity of the runtime provider registered by the Rust composition root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_provider_id: Option<RuntimeProviderId>,
    /// Authoritative current core readiness.
    pub readiness: ReadinessReport,
    /// Opaque identifier for this sidecar process instance.
    pub instance_id: InstanceId,
    /// Correlation identifier copied from the client hello.
    pub correlation_id: CorrelationId,
    /// Request identifier copied from the client hello.
    pub request_id: RequestId,
}

/// Authenticated request for current platform status.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct HealthRequest {
    /// Identifier shared by the response and any safe failure.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Current authoritative core status with boundary identifiers.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Version and readiness reported by the running platform core.
    pub status: PlatformStatus,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// One-line secret bootstrap payload written through the inherited stdin pipe.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct BootstrapRequest {
    /// Hex-encoded 256-bit per-launch bearer token.
    pub bearer_token: String,
    /// Oldest protocol version accepted by the launching desktop.
    pub protocol_min: u32,
    /// Newest protocol version accepted by the launching desktop.
    pub protocol_max: u32,
    /// Process identifier of the supervising desktop process.
    pub supervisor_process_id: u32,
    /// Opaque nonce unique to this supervision session.
    pub supervision_nonce: SupervisionNonce,
}

/// Public bootstrap result returned through stdout after loopback binding.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct BootstrapReady {
    /// Schema generation used by this bootstrap response.
    pub schema_version: u32,
    /// Dynamically assigned loopback TCP port.
    pub port: u16,
    /// Opaque identifier for the running sidecar instance.
    pub instance_id: InstanceId,
    /// Oldest protocol version supported by the sidecar.
    pub protocol_min: u32,
    /// Newest protocol version supported by the sidecar.
    pub protocol_max: u32,
}

impl BootstrapReady {
    /// Creates a public bootstrap response without exposing the bearer token.
    #[must_use]
    pub const fn new(
        port: u16,
        instance_id: InstanceId,
        protocol_min: u32,
        protocol_max: u32,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            port,
            instance_id,
            protocol_min,
            protocol_max,
        }
    }
}

/// Authenticated request to start the deterministic foundation event operation.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct TestOperationStartRequest {
    /// Identifier shared by all events and safe failures for the operation.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this start request.
    pub request_id: RequestId,
}

/// Accepted deterministic foundation operation.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct TestOperationStartResponse {
    /// Opaque identifier used for event subscription and cancellation.
    pub operation_id: OperationId,
    /// Identifier copied from the start request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the start request.
    pub request_id: RequestId,
}

/// Stable kinds emitted by the deterministic foundation event operation.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum TestOperationEventKind {
    /// The operation was accepted and began.
    Started,
    /// One deterministic progress step completed.
    Progress,
    /// The operation reached its successful terminal state.
    Completed,
    /// The operation observed explicit cancellation.
    Cancelled,
    /// The operation reached an explicit failed terminal state.
    Failed,
    /// The operation exceeded its bounded deadline.
    TimedOut,
    /// A newer peer supplied an event kind this build does not recognize.
    #[serde(other)]
    Unknown,
}

/// Terminal result carried by the final deterministic operation event.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum TestOperationTerminalState {
    /// All deterministic steps completed.
    Completed,
    /// Explicit cancellation was observed.
    Cancelled,
    /// The operation failed safely.
    Failed,
    /// The operation exceeded its deadline.
    TimedOut,
    /// A newer peer supplied a terminal state this build does not recognize.
    #[serde(other)]
    Unknown,
}

/// Ordered provider-neutral event used to prove the Task 04 stream foundation.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct TestOperationEvent {
    /// Schema generation for this event payload.
    pub schema_version: u32,
    /// Operation that owns this event stream.
    pub operation_id: OperationId,
    /// Identifier shared by every event for the operation.
    pub correlation_id: CorrelationId,
    /// Monotonically increasing sequence number starting at one.
    pub sequence: u64,
    /// Stable event classification.
    pub kind: TestOperationEventKind,
    /// Milliseconds since the Unix epoch, captured by the host.
    pub timestamp_unix_ms: u64,
    /// Optional safe provider-neutral progress explanation.
    pub message: Option<String>,
    /// Terminal state present only on the final event.
    pub terminal_state: Option<TestOperationTerminalState>,
}

/// Authenticated explicit cancellation request.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct CancelOperationRequest {
    /// Identifier shared with the operation being cancelled.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this cancellation request.
    pub request_id: RequestId,
}

/// Result of accepting or observing an explicit cancellation request.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct CancelOperationResponse {
    /// Operation targeted by the cancellation request.
    pub operation_id: OperationId,
    /// Whether the host accepted cancellation for a non-terminal operation.
    pub accepted: bool,
    /// Identifier copied from the cancellation request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the cancellation request.
    pub request_id: RequestId,
}

/// Authenticated request for bounded sidecar shutdown.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ShutdownRequest {
    /// Identifier shared with the response and any safe failure.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this shutdown request.
    pub request_id: RequestId,
}

/// Acknowledgement that bounded shutdown was accepted.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ShutdownResponse {
    /// Whether the host accepted the shutdown signal.
    pub accepted: bool,
    /// Identifier copied from the shutdown request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the shutdown request.
    pub request_id: RequestId,
}

macro_rules! opaque_uuid_id {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(uuid::Uuid);

        impl $name {
            /// Generates a random opaque version-four UUID.
            #[must_use]
            pub fn new() -> Self {
                Self(uuid::Uuid::new_v4())
            }

            /// Creates the identifier from a UUID for deterministic tests.
            #[must_use]
            pub const fn from_uuid(value: uuid::Uuid) -> Self {
                Self(value)
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl std::str::FromStr for $name {
            type Err = uuid::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                uuid::Uuid::parse_str(value).map(Self)
            }
        }
    };
}

opaque_uuid_id!(
    InstanceId,
    "Opaque identifier for one running core instance."
);
opaque_uuid_id!(OperationId, "Opaque identifier for one core operation.");
opaque_uuid_id!(
    SupervisionNonce,
    "Opaque nonce tying a sidecar to one desktop supervision session."
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_capabilities_remain_forward_compatible() {
        let capability: TransportCapability =
            serde_json::from_str("\"future_capability\"").expect("unknown capability is accepted");

        assert_eq!(capability, TransportCapability::Unknown);
    }

    #[test]
    fn bootstrap_ready_contains_only_public_process_metadata() {
        let instance_id = InstanceId::from_uuid(uuid::Uuid::nil());
        let ready = BootstrapReady::new(48_123, instance_id, 1, 1);
        let json = serde_json::to_value(ready).expect("bootstrap response serializes");

        assert_eq!(json["port"], 48_123);
        assert_eq!(json["schema_version"], SCHEMA_VERSION);
        assert!(json.get("bearer_token").is_none());
    }
}
