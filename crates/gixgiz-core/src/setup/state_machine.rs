use gixgiz_persistence::{PersistedSetupStage, PersistedSetupState};

use crate::CoreError;

pub(super) fn validate_transition(
    from: PersistedSetupState,
    to: PersistedSetupState,
    stage: PersistedSetupStage,
) -> Result<(), CoreError> {
    if stage_for(to) != stage || !transition_allowed(from, to) {
        return Err(CoreError::SetupStateConflict);
    }
    Ok(())
}

pub(super) const fn stage_for(state: PersistedSetupState) -> PersistedSetupStage {
    match state {
        PersistedSetupState::DraftPlan => PersistedSetupStage::PlanReview,
        PersistedSetupState::AwaitingApproval => PersistedSetupStage::Approval,
        PersistedSetupState::Approved | PersistedSetupState::Preparing => {
            PersistedSetupStage::Preparation
        }
        PersistedSetupState::CheckingStorage => PersistedSetupStage::Storage,
        PersistedSetupState::Acquiring => PersistedSetupStage::Acquisition,
        PersistedSetupState::Registering => PersistedSetupStage::Registration,
        PersistedSetupState::VerifyingRuntime => PersistedSetupStage::RuntimeVerification,
        PersistedSetupState::VerifyingModel => PersistedSetupStage::ModelVerification,
        PersistedSetupState::RunningTestInference => PersistedSetupStage::InferenceVerification,
        PersistedSetupState::Ready => PersistedSetupStage::Complete,
        PersistedSetupState::AttentionRequired => PersistedSetupStage::Attention,
        PersistedSetupState::Failed => PersistedSetupStage::Failed,
        PersistedSetupState::Cancelled => PersistedSetupStage::Cancelled,
        _ => PersistedSetupStage::Attention,
    }
}

const fn transition_allowed(from: PersistedSetupState, to: PersistedSetupState) -> bool {
    if matches!(
        to,
        PersistedSetupState::AttentionRequired
            | PersistedSetupState::Failed
            | PersistedSetupState::Cancelled
    ) {
        return !matches!(
            from,
            PersistedSetupState::Ready
                | PersistedSetupState::Failed
                | PersistedSetupState::Cancelled
        );
    }
    matches!(
        (from, to),
        (
            PersistedSetupState::AwaitingApproval,
            PersistedSetupState::Approved
        ) | (
            PersistedSetupState::Approved,
            PersistedSetupState::Preparing
        ) | (
            PersistedSetupState::Preparing,
            PersistedSetupState::CheckingStorage
        ) | (
            PersistedSetupState::CheckingStorage,
            PersistedSetupState::Acquiring
        ) | (
            PersistedSetupState::Acquiring,
            PersistedSetupState::Acquiring
        ) | (
            PersistedSetupState::Acquiring,
            PersistedSetupState::Registering
        ) | (
            PersistedSetupState::Registering,
            PersistedSetupState::VerifyingRuntime
        ) | (
            PersistedSetupState::VerifyingRuntime,
            PersistedSetupState::VerifyingModel
        ) | (
            PersistedSetupState::VerifyingModel,
            PersistedSetupState::RunningTestInference
        ) | (
            PersistedSetupState::RunningTestInference,
            PersistedSetupState::Ready
        ) | (
            PersistedSetupState::AttentionRequired
                | PersistedSetupState::Failed
                | PersistedSetupState::Cancelled,
            PersistedSetupState::Preparing
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_is_explicit_and_ready_cannot_be_skipped_to() {
        assert!(
            validate_transition(
                PersistedSetupState::Approved,
                PersistedSetupState::Preparing,
                PersistedSetupStage::Preparation,
            )
            .is_ok()
        );
        assert_eq!(
            validate_transition(
                PersistedSetupState::Preparing,
                PersistedSetupState::Ready,
                PersistedSetupStage::Complete,
            ),
            Err(CoreError::SetupStateConflict)
        );
    }

    #[test]
    fn retry_is_explicit_but_ready_is_terminal() {
        assert!(
            validate_transition(
                PersistedSetupState::AttentionRequired,
                PersistedSetupState::Preparing,
                PersistedSetupStage::Preparation,
            )
            .is_ok()
        );
        assert_eq!(
            validate_transition(
                PersistedSetupState::Ready,
                PersistedSetupState::Preparing,
                PersistedSetupStage::Preparation,
            ),
            Err(CoreError::SetupStateConflict)
        );
    }
}
