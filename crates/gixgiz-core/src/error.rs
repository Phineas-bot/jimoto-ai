use gixgiz_contracts::{ErrorCategory, RecoveryAction, RecoveryGuidance, SafeErrorPayload};
use thiserror::Error;

use crate::{CoreLifecycle, OperationContext};

/// Typed failures produced by the Task 03 platform-core foundation.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CoreError {
    /// Cooperative cancellation was observed.
    #[error("the operation was cancelled")]
    Cancelled,
    /// The operation deadline elapsed.
    #[error("the operation timed out")]
    TimedOut,
    /// A lifecycle transition was requested from an incompatible state.
    #[error("the requested lifecycle operation is not valid in the current state")]
    LifecycleConflict {
        /// Stable name of the requested lifecycle operation.
        operation: &'static str,
        /// Lifecycle state observed when the request was rejected.
        state: CoreLifecycle,
    },
    /// A service-health dependency failed to return safe evidence.
    #[error("service health collection failed")]
    HealthCollection {
        /// Stable internal identifier of the failing service.
        service_id: String,
    },
    /// The current user could not access the hardware evidence provider.
    #[error("hardware evidence access was denied")]
    HardwarePermissionDenied,
    /// The supported Windows hardware evidence provider is unavailable.
    #[error("the hardware evidence provider is unavailable")]
    HardwareProviderUnavailable,
    /// Structured hardware provider output failed bounded validation.
    #[error("the hardware evidence provider returned invalid data")]
    HardwareProviderInvalidData,
    /// Hardware provider output exceeded its fixed size limit.
    #[error("the hardware evidence provider exceeded its output limit")]
    HardwareProviderOutputLimit,
    /// The supplied machine profile uses an unsupported schema generation.
    #[error("the machine profile schema is not supported")]
    UnsupportedMachineProfile {
        /// Schema version supplied by the caller.
        received: u32,
    },
    /// The supplied recommendation preference enum is unknown to this rule set.
    #[error("the recommendation preferences are not supported")]
    InvalidRecommendationPreferences,
}

impl CoreError {
    /// Maps internal failures to stable payloads that contain no raw causes.
    #[must_use]
    pub fn to_safe_payload(&self, context: &OperationContext) -> SafeErrorPayload {
        let (category, code, message, recovery) = match self {
            Self::Cancelled => (
                ErrorCategory::Cancelled,
                "core.operation_cancelled",
                "The operation was cancelled.",
                RecoveryGuidance {
                    action: RecoveryAction::NoAction,
                    message: "No further action is required.".to_owned(),
                },
            ),
            Self::TimedOut => (
                ErrorCategory::TimedOut,
                "core.operation_timed_out",
                "The platform core did not finish the operation in time.",
                RecoveryGuidance {
                    action: RecoveryAction::Retry,
                    message: "Retry the operation.".to_owned(),
                },
            ),
            Self::LifecycleConflict { .. } => (
                ErrorCategory::Conflict,
                "core.lifecycle_conflict",
                "The platform core cannot perform that operation in its current state.",
                RecoveryGuidance {
                    action: RecoveryAction::Restart,
                    message: "Restart the platform core and retry.".to_owned(),
                },
            ),
            Self::HealthCollection { .. } => (
                ErrorCategory::Unavailable,
                "core.health_collection_failed",
                "The platform core could not collect required service health.",
                RecoveryGuidance {
                    action: RecoveryAction::Retry,
                    message: "Retry the health check.".to_owned(),
                },
            ),
            Self::HardwarePermissionDenied => (
                ErrorCategory::PermissionDenied,
                "hardware.permission_denied",
                "GixGiz could not access hardware evidence for the current user.",
                RecoveryGuidance {
                    action: RecoveryAction::Retry,
                    message: "Retry the scan without changing system permissions.".to_owned(),
                },
            ),
            Self::HardwareProviderUnavailable => (
                ErrorCategory::Unavailable,
                "hardware.provider_unavailable",
                "The Windows hardware evidence provider is unavailable.",
                RecoveryGuidance {
                    action: RecoveryAction::Retry,
                    message: "Retry the scan after Windows system services are available."
                        .to_owned(),
                },
            ),
            Self::HardwareProviderInvalidData => (
                ErrorCategory::Degraded,
                "hardware.provider_data_invalid",
                "Windows returned hardware evidence that GixGiz could not validate.",
                RecoveryGuidance {
                    action: RecoveryAction::Retry,
                    message: "Retry the scan. Unknown values will remain explicit.".to_owned(),
                },
            ),
            Self::HardwareProviderOutputLimit => (
                ErrorCategory::ResourceExhausted,
                "hardware.provider_output_limit",
                "The hardware provider returned more data than GixGiz accepts.",
                RecoveryGuidance {
                    action: RecoveryAction::Retry,
                    message: "Retry the scan. If it repeats, review the safe diagnostics."
                        .to_owned(),
                },
            ),
            Self::UnsupportedMachineProfile { .. } => (
                ErrorCategory::IncompatibleVersion,
                "capability.machine_profile_incompatible",
                "The hardware profile uses an unsupported schema version.",
                RecoveryGuidance {
                    action: RecoveryAction::Retry,
                    message: "Run a new hardware scan, then retry the recommendation.".to_owned(),
                },
            ),
            Self::InvalidRecommendationPreferences => (
                ErrorCategory::InvalidInput,
                "capability.preferences_invalid",
                "The selected recommendation preferences are not supported.",
                RecoveryGuidance {
                    action: RecoveryAction::Retry,
                    message: "Choose a supported workload and priority, then retry.".to_owned(),
                },
            ),
        };

        SafeErrorPayload::new(
            category,
            code,
            message,
            recovery,
            context.correlation_id(),
            context.request_id(),
        )
    }
}
