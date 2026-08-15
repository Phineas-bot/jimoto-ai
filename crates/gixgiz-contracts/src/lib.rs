//! Versioned, provider-neutral contracts shared by GixGiz processes.
//!
//! This crate contains data and stable boundary semantics only. It does not own
//! readiness policy, system access, persistence, transport, or provider logic.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod capability;
mod error;
mod hardware;
mod identity;
mod ids;
mod readiness;
mod runtime;
mod setup;
mod transport;

pub use capability::{
    CAPABILITY_REPORT_SCHEMA_VERSION, CandidateModel, CandidateModelId, CandidateRuntime,
    CandidateRuntimeId, CapabilityReport, CapabilityReportStatus, CatalogueVersion,
    CompatibilityStatus, ConfidenceLevel, MemoryEstimate, ModelSizeClass, NoPlanResult, PlanRole,
    PreferencePriority, RecommendationPlan, RecommendationReason, RecommendationReasonCode,
    RecommendationRequest, RecommendationResponse, RecommendationWarning,
    RecommendationWarningCode, ResourceEstimate, RuleSetVersion, StorageEstimate,
    UserPreferenceProfile, WorkloadTier,
};
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
pub use runtime::{
    RUNTIME_REPORT_SCHEMA_VERSION, RuntimeCapabilityAvailability, RuntimeCapabilityDescriptor,
    RuntimeCapabilityKind, RuntimeConsentDecision, RuntimeConsentRequest, RuntimeConsentResponse,
    RuntimeConsentState, RuntimeDisplayName, RuntimeEndpointSafety, RuntimeErrorCode,
    RuntimeHealthReport, RuntimeModelInventory, RuntimeModelInventoryRequest,
    RuntimeModelInventoryResponse, RuntimeModelMappingStatus, RuntimeModelSummary,
    RuntimeOperationEvent, RuntimeOperationEventKind, RuntimeOperationKind,
    RuntimeOperationStartRequest, RuntimeOperationStartResponse, RuntimeOperationTerminalState,
    RuntimeOwnership, RuntimeProviderId, RuntimeProviderModelId, RuntimeProviderModelMapping,
    RuntimeReason, RuntimeReasonCode, RuntimeState, RuntimeStatusRequest, RuntimeStatusResponse,
    RuntimeVersionCompatibility, RuntimeVersionInfo, RuntimeWarning, RuntimeWarningCode,
};
pub use setup::{
    ModelAcquisitionPhase, ModelAcquisitionProgress, ModelIntegrityState, ModelLifecycleState,
    ModelMetadata, ModelProviderArtifact, ModelVerificationResult, ModelVerificationState,
    ProviderRegistrationResult, ProviderRegistrationState, SETUP_WORKFLOW_SCHEMA_VERSION,
    SetupApprovalDecision, SetupApprovalRecord, SetupApprovalRequest, SetupApprovalResponse,
    SetupAttentionReason, SetupCancellationReport, SetupDestinationCategory, SetupEffect,
    SetupEffectDisposition, SetupEffectKind, SetupEffectReport, SetupJobCancelRequest,
    SetupJobCancelResponse, SetupJobEvent, SetupJobEventKind, SetupJobEventsRequest,
    SetupJobEventsResponse, SetupJobId, SetupJobRecoveryRequest, SetupJobRecoveryResponse,
    SetupJobRetryRequest, SetupJobRetryResponse, SetupJobSnapshot, SetupJobStartRequest,
    SetupJobStartResponse, SetupJobState, SetupJobStatusRequest, SetupJobStatusResponse,
    SetupJobTerminalState, SetupPlan, SetupPlanComponent, SetupPlanComponentKind, SetupPlanRequest,
    SetupPlanResponse, SetupReason, SetupReasonCode, SetupRecoveryAction, SetupStage, SetupWarning,
    SetupWarningCode,
};
pub use transport::{
    BootstrapReady, BootstrapRequest, CancelOperationRequest, CancelOperationResponse, ClientHello,
    CoreHello, HealthRequest, HealthResponse, InstanceId, OperationId, ShutdownRequest,
    ShutdownResponse, SupervisionNonce, TestOperationEvent, TestOperationEventKind,
    TestOperationStartRequest, TestOperationStartResponse, TestOperationTerminalState,
    TransportCapability,
};
