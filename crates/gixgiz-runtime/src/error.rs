use gixgiz_contracts::{
    ErrorCategory, RecoveryAction, RecoveryGuidance, RuntimeErrorCode, SafeErrorPayload,
};
use thiserror::Error;

use crate::RuntimeOperationContext;

/// Provider-neutral failures returned by runtime adapters and policy services.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeError {
    /// The request did not identify a supported provider, operation, or bound.
    #[error("the runtime request is invalid")]
    InvalidInput,
    /// Explicit cooperative cancellation was observed.
    #[error("the runtime operation was cancelled")]
    Cancelled,
    /// The bounded operation deadline elapsed.
    #[error("the runtime operation timed out")]
    TimedOut,
    /// No validated provider installation is available.
    #[error("the runtime is not installed")]
    NotInstalled,
    /// The provider executable failed the platform identity policy.
    #[error("the runtime executable is not trusted")]
    ExecutableUntrusted,
    /// The provider endpoint is not restricted to an approved loopback address.
    #[error("the runtime endpoint is unsafe")]
    EndpointUnsafe,
    /// The provider is currently unavailable.
    #[error("the runtime provider is unavailable")]
    ProviderUnavailable,
    /// Provider data failed validation or a fixed bound.
    #[error("the runtime provider response is invalid")]
    InvalidResponse,
    /// Required provider capabilities are incompatible.
    #[error("the runtime provider is incompatible")]
    IncompatibleVersion,
    /// The provider cannot safely perform this operation.
    #[error("the runtime operation is not supported")]
    Unsupported,
    /// Explicit management or reuse consent is missing.
    #[error("runtime consent is required")]
    ConsentRequired,
    /// Current ownership conflicts with the requested operation.
    #[error("runtime ownership does not permit the operation")]
    OwnershipConflict,
    /// A lifecycle operation is already active.
    #[error("another runtime operation is active")]
    Busy,
    /// A managed provider process failed without exposing provider output.
    #[error("the managed runtime process failed")]
    ProcessFailed,
    /// The canonical model has no validated provider mapping.
    #[error("the model is not mapped for this runtime")]
    ModelNotMapped,
    /// Provider acquisition did not reach a validated terminal success.
    #[error("the model acquisition failed")]
    ModelAcquisitionFailed,
    /// The exact provider model is not locally available.
    #[error("the model is not available")]
    ModelUnavailable,
    /// Exact provider registration could not be established.
    #[error("the model registration could not be verified")]
    ModelRegistrationFailed,
    /// Available model integrity evidence did not match expectations.
    #[error("the model integrity check failed")]
    ModelIntegrityFailed,
    /// The provider-managed model destination is unavailable.
    #[error("the model storage destination is unavailable")]
    ModelStorageUnavailable,
    /// The model destination lacks required free space and margin.
    #[error("the model storage destination has insufficient space")]
    ModelStorageExhausted,
    /// The fixed bounded readiness inference failed.
    #[error("the model readiness inference failed")]
    ReadinessInferenceFailed,
    /// Provider output exceeded a fixed bound.
    #[error("runtime provider output exceeded its limit")]
    OutputLimit,
    /// Runtime policy persistence is unavailable.
    #[error("runtime policy storage is unavailable")]
    PolicyUnavailable,
    /// No safer stable error category applies.
    #[error("the runtime operation failed internally")]
    Internal,
}

impl RuntimeError {
    /// Returns the stable runtime-specific code for this failure.
    #[must_use]
    pub const fn code(&self) -> RuntimeErrorCode {
        match self {
            Self::InvalidInput => RuntimeErrorCode::InvalidRequest,
            Self::Cancelled => RuntimeErrorCode::Cancelled,
            Self::TimedOut => RuntimeErrorCode::TimedOut,
            Self::NotInstalled => RuntimeErrorCode::NotInstalled,
            Self::ExecutableUntrusted => RuntimeErrorCode::ExecutableUntrusted,
            Self::EndpointUnsafe => RuntimeErrorCode::EndpointUnsafe,
            Self::ProviderUnavailable | Self::PolicyUnavailable => {
                RuntimeErrorCode::ProviderUnavailable
            }
            Self::InvalidResponse => RuntimeErrorCode::ProviderResponseInvalid,
            Self::IncompatibleVersion => RuntimeErrorCode::VersionIncompatible,
            Self::Unsupported => RuntimeErrorCode::OperationUnsupported,
            Self::ConsentRequired => RuntimeErrorCode::ConsentRequired,
            Self::OwnershipConflict => RuntimeErrorCode::OwnershipConflict,
            Self::Busy => RuntimeErrorCode::OperationBusy,
            Self::ProcessFailed => RuntimeErrorCode::ProcessFailed,
            Self::ModelNotMapped => RuntimeErrorCode::ModelNotMapped,
            Self::ModelAcquisitionFailed => RuntimeErrorCode::ModelAcquisitionFailed,
            Self::ModelUnavailable => RuntimeErrorCode::ModelUnavailable,
            Self::ModelRegistrationFailed => RuntimeErrorCode::ModelRegistrationFailed,
            Self::ModelIntegrityFailed => RuntimeErrorCode::ModelIntegrityFailed,
            Self::ModelStorageUnavailable => RuntimeErrorCode::ModelStorageUnavailable,
            Self::ModelStorageExhausted => RuntimeErrorCode::ModelStorageExhausted,
            Self::ReadinessInferenceFailed => RuntimeErrorCode::ReadinessInferenceFailed,
            Self::OutputLimit => RuntimeErrorCode::OutputLimit,
            Self::Internal => RuntimeErrorCode::Internal,
        }
    }

    /// Maps the failure to an existing safe cross-process payload.
    #[must_use]
    pub fn to_safe_payload(&self, context: &RuntimeOperationContext) -> SafeErrorPayload {
        let (category, code, message, action, recovery) = match self {
            Self::InvalidInput => (
                ErrorCategory::InvalidInput,
                "runtime.request_invalid",
                "The runtime request is not supported.",
                RecoveryAction::Retry,
                "Retry using an action offered by GixGiz.",
            ),
            Self::Cancelled => (
                ErrorCategory::Cancelled,
                "runtime.operation_cancelled",
                "The runtime operation was cancelled.",
                RecoveryAction::NoAction,
                "No further action is required.",
            ),
            Self::TimedOut => (
                ErrorCategory::TimedOut,
                "runtime.operation_timed_out",
                "The runtime did not finish the operation in time.",
                RecoveryAction::Retry,
                "Check the runtime status, then retry.",
            ),
            Self::NotInstalled => (
                ErrorCategory::Unavailable,
                "runtime.not_installed",
                "No compatible local runtime installation was found.",
                RecoveryAction::CheckPrerequisites,
                "Review the local runtime prerequisites.",
            ),
            Self::ExecutableUntrusted => (
                ErrorCategory::IntegrityFailure,
                "runtime.executable_untrusted",
                "The local runtime executable did not pass identity verification.",
                RecoveryAction::CheckPrerequisites,
                "Use a trusted signed runtime build, then retry.",
            ),
            Self::EndpointUnsafe => (
                ErrorCategory::PermissionDenied,
                "runtime.endpoint_unsafe",
                "The runtime endpoint is not restricted to this computer.",
                RecoveryAction::CheckPrerequisites,
                "Use a loopback-only runtime endpoint, then retry.",
            ),
            Self::ProviderUnavailable => (
                ErrorCategory::Unavailable,
                "runtime.provider_unavailable",
                "The local runtime provider is unavailable.",
                RecoveryAction::Retry,
                "Check the runtime status, then retry.",
            ),
            Self::InvalidResponse | Self::OutputLimit => (
                ErrorCategory::Degraded,
                "runtime.provider_response_invalid",
                "The runtime returned data that GixGiz could not validate.",
                RecoveryAction::Retry,
                "Retry the operation. If it repeats, review safe diagnostics.",
            ),
            Self::IncompatibleVersion => (
                ErrorCategory::IncompatibleVersion,
                "runtime.version_incompatible",
                "The installed runtime is not compatible with this GixGiz build.",
                RecoveryAction::CheckPrerequisites,
                "Review the supported runtime requirements.",
            ),
            Self::Unsupported => (
                ErrorCategory::NotSupported,
                "runtime.operation_unsupported",
                "This runtime operation is not supported safely.",
                RecoveryAction::NoAction,
                "Use one of the actions offered by GixGiz.",
            ),
            Self::ConsentRequired => (
                ErrorCategory::PermissionDenied,
                "runtime.consent_required",
                "Explicit runtime consent is required for this action.",
                RecoveryAction::NoAction,
                "Review the ownership and consent request before continuing.",
            ),
            Self::OwnershipConflict => (
                ErrorCategory::Conflict,
                "runtime.ownership_conflict",
                "GixGiz does not own this runtime lifecycle.",
                RecoveryAction::NoAction,
                "Use external runtime controls or grant management separately.",
            ),
            Self::Busy => (
                ErrorCategory::Conflict,
                "runtime.operation_busy",
                "Another runtime lifecycle operation is already active.",
                RecoveryAction::Retry,
                "Wait for the current operation or cancel it, then retry.",
            ),
            Self::ProcessFailed => (
                ErrorCategory::Unavailable,
                "runtime.process_failed",
                "The managed runtime process stopped unexpectedly.",
                RecoveryAction::Retry,
                "Check runtime status before retrying.",
            ),
            Self::ModelNotMapped => (
                ErrorCategory::InvalidInput,
                "runtime.model_not_mapped",
                "The selected model is not available for this runtime.",
                RecoveryAction::CheckPrerequisites,
                "Refresh the model recommendation, then review the new plan.",
            ),
            Self::ModelAcquisitionFailed => (
                ErrorCategory::Unavailable,
                "runtime.model_acquisition_failed",
                "The local model could not be prepared.",
                RecoveryAction::Retry,
                "Check storage and the runtime status, then retry.",
            ),
            Self::ModelUnavailable => (
                ErrorCategory::Unavailable,
                "runtime.model_unavailable",
                "The selected local model is not available.",
                RecoveryAction::Retry,
                "Retry model preparation or choose another recommended model.",
            ),
            Self::ModelRegistrationFailed => (
                ErrorCategory::Degraded,
                "runtime.model_registration_failed",
                "The local model registration could not be verified.",
                RecoveryAction::Retry,
                "Check the runtime status, then retry verification.",
            ),
            Self::ModelIntegrityFailed => (
                ErrorCategory::IntegrityFailure,
                "runtime.model_integrity_failed",
                "The local model did not pass integrity verification.",
                RecoveryAction::ContactSupport,
                "Do not use this model. Review sanitized diagnostics before trying again.",
            ),
            Self::ModelStorageUnavailable => (
                ErrorCategory::Unavailable,
                "runtime.model_storage_unavailable",
                "The local model storage destination is unavailable.",
                RecoveryAction::CheckPrerequisites,
                "Make the selected destination available, then retry.",
            ),
            Self::ModelStorageExhausted => (
                ErrorCategory::ResourceExhausted,
                "runtime.model_storage_exhausted",
                "There is not enough free space for the selected model.",
                RecoveryAction::CheckPrerequisites,
                "Free storage space or select a smaller recommended model.",
            ),
            Self::ReadinessInferenceFailed => (
                ErrorCategory::Degraded,
                "runtime.readiness_inference_failed",
                "The local model did not pass the readiness test.",
                RecoveryAction::Retry,
                "Check the runtime status, then retry model verification.",
            ),
            Self::PolicyUnavailable => (
                ErrorCategory::Unavailable,
                "runtime.policy_unavailable",
                "Runtime ownership policy is temporarily unavailable.",
                RecoveryAction::Restart,
                "Restart GixGiz, then retry.",
            ),
            Self::Internal => (
                ErrorCategory::Internal,
                "runtime.internal",
                "The runtime operation failed safely.",
                RecoveryAction::Restart,
                "Restart GixGiz, then retry.",
            ),
        };
        SafeErrorPayload::new(
            category,
            code,
            message,
            RecoveryGuidance {
                action,
                message: recovery.to_owned(),
            },
            context.correlation_id(),
            context.request_id(),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use gixgiz_contracts::{CorrelationId, RequestId};

    use super::*;

    #[test]
    fn raw_provider_details_cannot_enter_safe_error_mapping() {
        let context = RuntimeOperationContext::new(
            CorrelationId::new(),
            RequestId::new(),
            Duration::from_secs(1),
        );
        let payload = RuntimeError::ProcessFailed.to_safe_payload(&context);
        let encoded = serde_json::to_string(&payload).expect("payload serializes");

        assert_eq!(payload.code, "runtime.process_failed");
        assert!(!encoded.contains("stderr"));
        assert!(!encoded.contains("command"));
    }

    #[test]
    fn untrusted_executable_maps_to_integrity_failure_without_provider_details() {
        let context = RuntimeOperationContext::new(
            CorrelationId::new(),
            RequestId::new(),
            Duration::from_secs(1),
        );
        let payload = RuntimeError::ExecutableUntrusted.to_safe_payload(&context);

        assert_eq!(payload.category, ErrorCategory::IntegrityFailure);
        assert_eq!(payload.code, "runtime.executable_untrusted");
        assert_eq!(payload.recovery.action, RecoveryAction::CheckPrerequisites);
        assert!(!payload.message.contains("Ollama"));
    }

    #[test]
    fn model_setup_failures_preserve_stable_categories_and_recovery() {
        let context = RuntimeOperationContext::new(
            CorrelationId::new(),
            RequestId::new(),
            Duration::from_secs(1),
        );
        let storage = RuntimeError::ModelStorageExhausted.to_safe_payload(&context);
        let integrity = RuntimeError::ModelIntegrityFailed.to_safe_payload(&context);
        let readiness = RuntimeError::ReadinessInferenceFailed.to_safe_payload(&context);

        assert_eq!(storage.category, ErrorCategory::ResourceExhausted);
        assert_eq!(storage.code, "runtime.model_storage_exhausted");
        assert_eq!(integrity.category, ErrorCategory::IntegrityFailure);
        assert_eq!(integrity.code, "runtime.model_integrity_failed");
        assert_eq!(readiness.category, ErrorCategory::Degraded);
        assert_eq!(readiness.code, "runtime.readiness_inference_failed");
        assert_eq!(readiness.recovery.action, RecoveryAction::Retry);
    }
}
