//! Versioned, provider-neutral contracts shared by GixGiz processes.
//!
//! This crate contains data and stable boundary semantics only. It does not own
//! readiness policy, system access, persistence, transport, or provider logic.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod error;
mod hardware;
mod identity;
mod ids;
mod readiness;
mod transport;

pub use error::{ErrorCategory, RecoveryAction, RecoveryGuidance, SafeErrorPayload};
pub use hardware::{
    AccelerationEvidence, AccelerationKind, ArchitectureEvidence, CpuEvidence,
    EvidenceAvailability, EvidenceConfidence, EvidenceMetadata, EvidenceSource,
    GpuCollectionEvidence, GpuEvidence, HardwareScanEvent, HardwareScanEventKind,
    HardwareScanStartRequest, HardwareScanStartResponse, HardwareScanTerminalState,
    MACHINE_PROFILE_SCHEMA_VERSION, MachineArchitecture, MachineProfile,
    MachineProfileCompleteness, OperatingSystemEvidence, PhysicalMemoryEvidence, StorageEvidence,
    StorageLocation, StorageMediaEvidence, StorageMediaKind, StringEvidence, U32Evidence,
    U64Evidence, UnknownReasonCode,
};
pub use identity::{
    APPLICATION_ID, APPLICATION_NAME, APPLICATION_VERSION, ApplicationInfo, PROTOCOL_VERSION,
    PlatformStatus, SCHEMA_VERSION,
};
pub use ids::{CorrelationId, RequestId};
pub use readiness::{
    ReadinessReport, ReadinessStatus, ServiceHealth, ServiceHealthStatus, ServiceRequirement,
};
pub use transport::{
    BootstrapReady, BootstrapRequest, CancelOperationRequest, CancelOperationResponse, ClientHello,
    CoreHello, HealthRequest, HealthResponse, InstanceId, OperationId, ShutdownRequest,
    ShutdownResponse, SupervisionNonce, TestOperationEvent, TestOperationEventKind,
    TestOperationStartRequest, TestOperationStartResponse, TestOperationTerminalState,
    TransportCapability,
};
