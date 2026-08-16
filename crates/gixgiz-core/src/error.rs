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
    /// Setup-critical recommendation or provider mapping fields failed revalidation.
    #[error("the setup plan is invalid")]
    InvalidSetupPlan,
    /// Durable setup persistence is unavailable.
    #[error("durable setup persistence is unavailable")]
    SetupPersistenceUnavailable,
    /// The requested setup job does not exist.
    #[error("the setup job was not found")]
    SetupJobNotFound,
    /// The setup job changed or is not valid for the requested transition.
    #[error("the setup job state conflicts with the request")]
    SetupStateConflict,
    /// The exact current setup plan has not been approved.
    #[error("the setup plan requires exact approval")]
    SetupApprovalRequired,
    /// The selected runtime provider cannot supply safe setup evidence.
    #[error("the setup runtime provider is unavailable")]
    SetupProviderUnavailable,
    /// Local chat could not proceed for a stable contract-defined reason.
    #[error("local chat could not proceed")]
    Chat(gixgiz_contracts::ChatFailureCode),
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
            Self::InvalidSetupPlan => (
                ErrorCategory::InvalidInput,
                "setup.plan_invalid",
                "The selected setup plan is no longer valid.",
                RecoveryGuidance {
                    action: RecoveryAction::Retry,
                    message: "Refresh the capability recommendation and review a new plan."
                        .to_owned(),
                },
            ),
            Self::SetupPersistenceUnavailable => (
                ErrorCategory::Unavailable,
                "setup.persistence_unavailable",
                "Durable setup state is unavailable.",
                RecoveryGuidance {
                    action: RecoveryAction::Restart,
                    message: "Restart GixGiz, then retry setup.".to_owned(),
                },
            ),
            Self::SetupJobNotFound => (
                ErrorCategory::InvalidInput,
                "setup.job_not_found",
                "The requested setup job was not found.",
                RecoveryGuidance {
                    action: RecoveryAction::Retry,
                    message: "Refresh setup status before retrying.".to_owned(),
                },
            ),
            Self::SetupStateConflict => (
                ErrorCategory::Conflict,
                "setup.state_conflict",
                "Setup changed before the request completed.",
                RecoveryGuidance {
                    action: RecoveryAction::Retry,
                    message: "Refresh the current setup state, then retry.".to_owned(),
                },
            ),
            Self::SetupApprovalRequired => (
                ErrorCategory::PermissionDenied,
                "setup.approval_required",
                "The exact current setup plan requires approval.",
                RecoveryGuidance {
                    action: RecoveryAction::NoAction,
                    message: "Review the current plan and its effects before continuing."
                        .to_owned(),
                },
            ),
            Self::Chat(code) => return chat_payload(*code, context),
            Self::SetupProviderUnavailable => (
                ErrorCategory::Unavailable,
                "setup.provider_unavailable",
                "The local runtime provider cannot prepare this setup safely.",
                RecoveryGuidance {
                    action: RecoveryAction::CheckPrerequisites,
                    message: "Check the local runtime status, then refresh the setup plan."
                        .to_owned(),
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

/// Maps the stable chat taxonomy onto a safe boundary payload.
fn chat_payload(
    code: gixgiz_contracts::ChatFailureCode,
    context: &OperationContext,
) -> SafeErrorPayload {
    use gixgiz_contracts::ChatFailureCode as Code;

    let (category, stable, message, action, guidance) = match code {
        Code::RuntimeUnavailable => (
            ErrorCategory::Unavailable,
            "chat.runtime_unavailable",
            "The local runtime is not available for chat right now.",
            RecoveryAction::CheckPrerequisites,
            "Check the local runtime status, then try again.",
        ),
        Code::RuntimeIncompatible => (
            ErrorCategory::IncompatibleVersion,
            "chat.runtime_incompatible",
            "The installed local runtime is not compatible with chat.",
            RecoveryAction::CheckPrerequisites,
            "Review the runtime requirements before chatting.",
        ),
        Code::RuntimeConsentRequired => (
            ErrorCategory::PermissionDenied,
            "chat.runtime_consent_required",
            "GixGiz needs your permission to use the existing local runtime.",
            RecoveryAction::CheckPrerequisites,
            "Approve reuse of the local runtime, then try again.",
        ),
        Code::ModelUnavailable => (
            ErrorCategory::Unavailable,
            "chat.model_unavailable",
            "The model for this conversation is no longer available.",
            RecoveryAction::CheckPrerequisites,
            "Run model setup again, then start a new message.",
        ),
        Code::ModelChanged => (
            ErrorCategory::Conflict,
            "chat.model_changed",
            "The model for this conversation changed since it was verified.",
            RecoveryAction::CheckPrerequisites,
            "Run model setup again to verify the model.",
        ),
        Code::GenerationTimedOut => (
            ErrorCategory::TimedOut,
            "chat.generation_timed_out",
            "The local model took too long to reply.",
            RecoveryAction::Retry,
            "Send the message again.",
        ),
        Code::ProviderDisconnected => (
            ErrorCategory::Degraded,
            "chat.provider_disconnected",
            "The local model stopped replying before it finished.",
            RecoveryAction::Retry,
            "Check the local runtime, then send the message again.",
        ),
        Code::MalformedProviderStream => (
            ErrorCategory::Degraded,
            "chat.malformed_provider_stream",
            "GixGiz could not read the local model's reply safely.",
            RecoveryAction::Retry,
            "Send the message again.",
        ),
        Code::MessageTooLarge => (
            ErrorCategory::InvalidInput,
            "chat.message_too_large",
            "That message is too long to send.",
            RecoveryAction::NoAction,
            "Shorten the message and send it again.",
        ),
        Code::ContextTooLarge => (
            ErrorCategory::ResourceExhausted,
            "chat.context_too_large",
            "This conversation is too long to continue safely.",
            RecoveryAction::NoAction,
            "Start a new conversation to continue.",
        ),
        Code::ConversationNotFound => (
            ErrorCategory::Unavailable,
            "chat.conversation_not_found",
            "That conversation is no longer available.",
            RecoveryAction::NoAction,
            "Choose another conversation or start a new one.",
        ),
        Code::GenerationNotFound => (
            ErrorCategory::Unavailable,
            "chat.generation_not_found",
            "That reply is no longer being generated.",
            RecoveryAction::NoAction,
            "Reload the conversation to see its current state.",
        ),
        Code::GenerationAlreadyActive => (
            ErrorCategory::Conflict,
            "chat.generation_already_active",
            "This conversation is already waiting for a reply.",
            RecoveryAction::Retry,
            "Wait for the current reply or stop it, then try again.",
        ),
        Code::PersistenceUnavailable => (
            ErrorCategory::Internal,
            "chat.persistence_unavailable",
            "GixGiz could not read or save this conversation.",
            RecoveryAction::Restart,
            "Restart GixGiz and try again.",
        ),
        Code::Cancelled => (
            ErrorCategory::Cancelled,
            "chat.cancelled",
            "The reply was stopped.",
            RecoveryAction::NoAction,
            "No further action is required.",
        ),
        _ => (
            ErrorCategory::Internal,
            "chat.unavailable",
            "GixGiz could not complete that chat action.",
            RecoveryAction::Retry,
            "Try again.",
        ),
    };

    SafeErrorPayload::new(
        category,
        stable,
        message,
        RecoveryGuidance {
            action,
            message: guidance.to_owned(),
        },
        context.correlation_id(),
        context.request_id(),
    )
}
