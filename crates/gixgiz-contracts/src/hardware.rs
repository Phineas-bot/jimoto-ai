use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{CorrelationId, OperationId, RequestId, SafeErrorPayload};

/// Schema generation for the first provider-neutral machine profile.
pub const MACHINE_PROFILE_SCHEMA_VERSION: u32 = 1;

/// Whether all requested hardware evidence was collected reliably.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum MachineProfileCompleteness {
    /// Every requested field has reliable available evidence.
    Complete,
    /// The profile is usable evidence, but at least one field is unavailable.
    Partial,
    /// A newer peer supplied an unrecognized completeness value.
    #[serde(other)]
    Unknown,
}

/// Availability of one observed hardware field or collection.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum EvidenceAvailability {
    /// A bounded value was collected from the stated source.
    Available,
    /// The current user could not access the evidence.
    PermissionDenied,
    /// Collection did not finish within its deadline.
    TimedOut,
    /// The provider does not support this evidence.
    Unsupported,
    /// The device or metric is not present.
    NotPresent,
    /// A value exists but is not reliable enough to report as available.
    NotReliable,
    /// The source could not determine availability or a newer peer supplied an unrecognized value.
    #[serde(other)]
    Unknown,
}

/// Confidence attached to one normalized evidence value.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum EvidenceConfidence {
    /// The source directly and reliably reported the value.
    High,
    /// The value is useful but has documented source limitations.
    Medium,
    /// The value is weak evidence and must not drive a decision alone.
    Low,
    /// Confidence is unavailable or was supplied by a newer peer.
    #[serde(other)]
    Unknown,
}

/// Provider-neutral source category for one hardware observation.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSource {
    /// Windows Common Information Model data accessed without elevation.
    WindowsCim,
    /// A bounded Windows process environment value.
    WindowsEnvironment,
    /// A normalized value derived only from other reported evidence.
    Derived,
    /// The source is unavailable or was supplied by a newer peer.
    #[serde(other)]
    Unknown,
}

/// Stable explanation category for unavailable hardware evidence.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum UnknownReasonCode {
    /// The provider did not report the requested field.
    NotReported,
    /// Access was denied to the current non-elevated user.
    AccessDenied,
    /// The provider exceeded the bounded collection deadline.
    DeadlineExceeded,
    /// The active provider cannot collect this field reliably.
    ProviderUnsupported,
    /// The requested device or metric is not present.
    DeviceAbsent,
    /// The source value is known to be unreliable for this purpose.
    SourceUnreliable,
    /// Structured provider data failed validation or normalization.
    InvalidProviderData,
    /// A newer peer supplied an unrecognized reason.
    #[serde(other)]
    Unknown,
}

/// Source, availability, confidence, and safe reason for one evidence value.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct EvidenceMetadata {
    /// Provider-neutral source category.
    pub source: EvidenceSource,
    /// Explicit availability state.
    pub availability: EvidenceAvailability,
    /// Confidence in the normalized value.
    pub confidence: EvidenceConfidence,
    /// Stable reason category when the value is unavailable.
    pub reason_code: Option<UnknownReasonCode>,
    /// Safe bounded explanation containing no raw provider output.
    pub reason: Option<String>,
}

impl EvidenceMetadata {
    /// Creates metadata for one available value.
    #[must_use]
    pub const fn available(source: EvidenceSource, confidence: EvidenceConfidence) -> Self {
        Self {
            source,
            availability: EvidenceAvailability::Available,
            confidence,
            reason_code: None,
            reason: None,
        }
    }

    /// Creates metadata for one unavailable value with an explicit safe reason.
    #[must_use]
    pub fn unavailable(
        source: EvidenceSource,
        availability: EvidenceAvailability,
        reason_code: UnknownReasonCode,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            source,
            availability,
            confidence: EvidenceConfidence::Unknown,
            reason_code: Some(reason_code),
            reason: Some(reason.into()),
        }
    }

    /// Returns whether this metadata describes a reliable available value.
    #[must_use]
    pub const fn is_available(&self) -> bool {
        matches!(self.availability, EvidenceAvailability::Available)
    }
}

/// Bounded text plus its collection metadata.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct StringEvidence {
    /// Normalized value when available.
    pub value: Option<String>,
    /// Source and availability metadata.
    pub metadata: EvidenceMetadata,
}

/// Unsigned 64-bit quantity plus its collection metadata.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct U64Evidence {
    /// Normalized value when available.
    pub value: Option<u64>,
    /// Source and availability metadata.
    pub metadata: EvidenceMetadata,
}

/// Unsigned 32-bit quantity plus its collection metadata.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct U32Evidence {
    /// Normalized value when available.
    pub value: Option<u32>,
    /// Source and availability metadata.
    pub metadata: EvidenceMetadata,
}

/// Normalized machine architecture.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum MachineArchitecture {
    /// 64-bit x86 architecture.
    X86_64,
    /// 64-bit Arm architecture.
    Arm64,
    /// 32-bit x86 architecture.
    X86,
    /// The architecture is unknown or was supplied by a newer peer.
    #[serde(other)]
    Unknown,
}

/// Machine architecture plus its collection metadata.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureEvidence {
    /// Normalized architecture when available.
    pub value: Option<MachineArchitecture>,
    /// Source and availability metadata.
    pub metadata: EvidenceMetadata,
}

/// Operating-system evidence safe to cross the desktop boundary.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct OperatingSystemEvidence {
    /// Product name reported by the operating system.
    pub name: StringEvidence,
    /// Product version reported by the operating system.
    pub version: StringEvidence,
    /// Product build reported by the operating system.
    pub build: StringEvidence,
    /// Normalized operating-system architecture.
    pub architecture: ArchitectureEvidence,
}

/// CPU identity and topology evidence.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct CpuEvidence {
    /// Bounded CPU product identity when consistently reported.
    pub name: StringEvidence,
    /// Bounded CPU vendor when consistently reported.
    pub vendor: StringEvidence,
    /// Sum of physical core counts across reported processor packages.
    pub physical_core_count: U32Evidence,
    /// Sum of logical processor counts across reported processor packages.
    pub logical_core_count: U32Evidence,
}

/// Physical-memory evidence, excluding virtual-memory totals.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct PhysicalMemoryEvidence {
    /// Total visible physical memory normalized to bytes.
    pub total_bytes: U64Evidence,
    /// Currently available physical memory normalized to bytes.
    pub available_bytes: U64Evidence,
}

/// Evidence for one display adapter without stable device identifiers.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct GpuEvidence {
    /// Bounded display-adapter name.
    pub name: StringEvidence,
    /// Bounded display-adapter vendor.
    pub vendor: StringEvidence,
    /// Dedicated video memory in bytes only when reliably obtainable.
    pub dedicated_memory_bytes: U64Evidence,
    /// Shared video memory in bytes only when reliably obtainable.
    pub shared_memory_bytes: U64Evidence,
}

/// Bounded GPU collection and collection-level availability.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct GpuCollectionEvidence {
    /// Display adapters reported by the provider, in stable provider order.
    pub devices: Vec<GpuEvidence>,
    /// Availability of the collection itself.
    pub metadata: EvidenceMetadata,
}

/// Acceleration API relevant to future local runtime readiness.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum AccelerationKind {
    /// Microsoft DirectML acceleration.
    DirectMl,
    /// NVIDIA CUDA acceleration.
    Cuda,
    /// AMD ROCm acceleration.
    Rocm,
    /// The acceleration API is unknown or was supplied by a newer peer.
    #[serde(other)]
    Unknown,
}

/// Explicit acceleration support evidence without marketing-name inference.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct AccelerationEvidence {
    /// Acceleration API being described.
    pub kind: AccelerationKind,
    /// Observed support when reliably probed.
    pub supported: Option<bool>,
    /// Source and availability metadata.
    pub metadata: EvidenceMetadata,
}

/// Privacy-preserving selected storage location category.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum StorageLocation {
    /// The per-user GixGiz application-data root.
    ApplicationData,
    /// A user-selected location represented without exposing its raw path.
    UserSelected,
    /// The location is unknown or was supplied by a newer peer.
    #[serde(other)]
    Unknown,
}

/// Normalized storage-media category.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum StorageMediaKind {
    /// Fixed local storage.
    Fixed,
    /// Removable storage.
    Removable,
    /// Network-backed storage.
    Network,
    /// Optical media.
    Optical,
    /// Memory-backed storage.
    RamDisk,
    /// The media kind is unknown or was supplied by a newer peer.
    #[serde(other)]
    Unknown,
}

/// Storage-media category plus collection metadata.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct StorageMediaEvidence {
    /// Normalized media kind when available.
    pub value: Option<StorageMediaKind>,
    /// Source and availability metadata.
    pub metadata: EvidenceMetadata,
}

/// Capacity evidence for the selected privacy-preserving storage location.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct StorageEvidence {
    /// Selected location category; no raw user path crosses the boundary.
    pub location: StorageLocation,
    /// Total capacity in bytes.
    pub capacity_bytes: U64Evidence,
    /// Currently free capacity in bytes.
    pub free_bytes: U64Evidence,
    /// Bounded filesystem name where safely available.
    pub filesystem: StringEvidence,
    /// Fixed, removable, or other normalized media evidence.
    pub media_kind: StorageMediaEvidence,
}

/// Versioned provider-neutral evidence profile produced by one hardware scan.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct MachineProfile {
    /// Machine-profile schema generation.
    pub schema_version: u32,
    /// Opaque scan operation identifier.
    pub scan_id: OperationId,
    /// Correlation identifier shared with the initiating request.
    pub correlation_id: CorrelationId,
    /// UTC Unix timestamp in milliseconds captured at scan completion.
    pub scanned_at_unix_ms: u64,
    /// Whether every requested field was collected reliably.
    pub completeness: MachineProfileCompleteness,
    /// Operating-system evidence.
    pub operating_system: OperatingSystemEvidence,
    /// CPU evidence.
    pub cpu: CpuEvidence,
    /// Physical-memory evidence.
    pub physical_memory: PhysicalMemoryEvidence,
    /// Display-adapter evidence.
    pub gpus: GpuCollectionEvidence,
    /// Explicit acceleration evidence without capability decisions.
    pub acceleration: Vec<AccelerationEvidence>,
    /// Evidence for the selected storage location.
    pub storage: StorageEvidence,
}

/// Authenticated request to start one hardware evidence scan.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct HardwareScanStartRequest {
    /// Identifier shared by the scan, events, and safe failures.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this start request.
    pub request_id: RequestId,
}

/// Accepted hardware scan operation.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct HardwareScanStartResponse {
    /// Opaque identifier used for event subscription and cancellation.
    pub operation_id: OperationId,
    /// Identifier copied from the start request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the start request.
    pub request_id: RequestId,
}

/// Stable kind emitted by one hardware scan operation.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum HardwareScanEventKind {
    /// The operation was accepted and began.
    Started,
    /// The provider is collecting bounded hardware evidence.
    Collecting,
    /// A complete or partial profile is available.
    Completed,
    /// Explicit cancellation was observed.
    Cancelled,
    /// Collection exceeded its bounded deadline.
    TimedOut,
    /// Collection failed without a usable profile.
    Failed,
    /// A newer peer supplied an unrecognized event kind.
    #[serde(other)]
    Unknown,
}

/// Terminal result for one hardware scan stream.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum HardwareScanTerminalState {
    /// All requested evidence was collected reliably.
    Completed,
    /// A usable profile contains explicitly unavailable evidence.
    Partial,
    /// Explicit cancellation was observed.
    Cancelled,
    /// Collection exceeded its bounded deadline.
    TimedOut,
    /// Collection failed without a usable profile.
    Failed,
    /// A newer peer supplied an unrecognized terminal state.
    #[serde(other)]
    Unknown,
}

/// Ordered event carrying progress or the final machine profile.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct HardwareScanEvent {
    /// Contract schema generation for this event.
    pub schema_version: u32,
    /// Operation that owns this event stream.
    pub operation_id: OperationId,
    /// Identifier shared by every event for the scan.
    pub correlation_id: CorrelationId,
    /// Monotonically increasing sequence number starting at one.
    pub sequence: u64,
    /// Stable event classification.
    pub kind: HardwareScanEventKind,
    /// UTC Unix timestamp in milliseconds captured by the host.
    pub timestamp_unix_ms: u64,
    /// Optional safe provider-neutral status explanation.
    pub message: Option<String>,
    /// Final profile present only for successful complete or partial scans.
    pub profile: Option<MachineProfile>,
    /// Safe typed failure present only for timed-out or failed terminal scans.
    pub error: Option<SafeErrorPayload>,
    /// Terminal state present only on the final event.
    pub terminal_state: Option<HardwareScanTerminalState>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_availability_remains_forward_compatible() {
        let availability: EvidenceAvailability =
            serde_json::from_str("\"future_state\"").expect("unknown availability is accepted");

        assert_eq!(availability, EvidenceAvailability::Unknown);
    }

    #[test]
    fn machine_profile_schema_excludes_stable_device_identifiers() {
        let schema = serde_json::to_string(&schemars::schema_for!(MachineProfile))
            .expect("machine profile schema serializes")
            .to_ascii_lowercase();

        for forbidden in [
            "serial_number",
            "serialnumber",
            "mac_address",
            "macaddress",
            "machine_guid",
            "pnp_device_id",
        ] {
            assert!(!schema.contains(forbidden), "schema contains {forbidden}");
        }
    }

    #[test]
    fn machine_profile_contract_version_is_stable() {
        assert_eq!(MACHINE_PROFILE_SCHEMA_VERSION, 1);
        let request = HardwareScanStartRequest {
            correlation_id: CorrelationId::new(),
            request_id: RequestId::new(),
        };
        let json = serde_json::to_value(request).expect("scan request serializes");
        assert!(json.get("correlation_id").is_some());
        assert!(json.get("request_id").is_some());
        assert_eq!(
            serde_json::to_string(&StorageLocation::UserSelected)
                .expect("storage location serializes"),
            "\"user_selected\""
        );
    }
}
