//! Pure transitions for the managed runtime-installation workflow.
//!
//! Every function here is deterministic and side-effect free so the whole
//! workflow can be reasoned about and tested without a provider, a network, a
//! database, administrator rights, or a real installer.
//!
//! Two invariants are enforced structurally rather than by convention:
//!
//! 1. no work may begin without an approval bound to the exact plan revision; and
//! 2. no outcome may become `Ready` without complete independent verification.

use gixgiz_contracts::{
    RuntimeInstallAttentionReason, RuntimeInstallEffect, RuntimeInstallEffectDisposition,
    RuntimeInstallEffectKind, RuntimeInstallEffectReport, RuntimeInstallFailureCode,
    RuntimeInstallRecoveryAction, RuntimeInstallStage, RuntimeInstallState,
    RuntimeInstallVerificationResult, RuntimeOwnership,
};
use gixgiz_runtime::{RuntimeArtifactRejection, RuntimeInstallExecution};

/// Maximum retries permitted for one approved plan revision.
pub const MAX_INSTALL_RETRIES: u32 = 3;

/// Decision about whether approved work may begin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StartDecision {
    /// Work may begin.
    Admit,
    /// Work may not begin; the job needs a user decision.
    Reject(RuntimeInstallAttentionReason),
}

/// Determines whether approved installation work may start.
///
/// An approval is valid only for the exact revision it names. A plan that was
/// regenerated after approval is a different plan and needs a new decision.
pub(super) fn decide_start(
    state: RuntimeInstallState,
    plan_revision: u32,
    approved_revision: Option<u32>,
) -> StartDecision {
    match approved_revision {
        Some(approved) if approved == plan_revision => match state {
            RuntimeInstallState::Approved | RuntimeInstallState::AttentionRequired => {
                StartDecision::Admit
            }
            _ => StartDecision::Reject(RuntimeInstallAttentionReason::ApprovalRequired),
        },
        _ => StartDecision::Reject(RuntimeInstallAttentionReason::ApprovalRequired),
    }
}

/// Maps an artifact rejection to its safe failure classification.
///
/// Every variant is terminal for the attempt: a rejected artifact is never
/// executed, so no path from here reaches installation.
pub(super) fn classify_rejection(rejection: RuntimeArtifactRejection) -> RuntimeInstallFailureCode {
    match rejection {
        RuntimeArtifactRejection::SourceUntrusted => RuntimeInstallFailureCode::SourceUntrusted,
        RuntimeArtifactRejection::TooLarge => RuntimeInstallFailureCode::ArtifactTooLarge,
        RuntimeArtifactRejection::DigestMismatch => RuntimeInstallFailureCode::IntegrityMismatch,
        RuntimeArtifactRejection::SignatureInvalid => RuntimeInstallFailureCode::SignatureInvalid,
        RuntimeArtifactRejection::PublisherUnexpected => {
            RuntimeInstallFailureCode::PublisherUnexpected
        }
        RuntimeArtifactRejection::InsufficientStagingSpace
        | RuntimeArtifactRejection::InsufficientInstallSpace => {
            RuntimeInstallFailureCode::InsufficientStorage
        }
        // A rejection kind added by a newer adapter is still a rejection: fail
        // closed rather than letting an unrecognized value fall through.
        _ => RuntimeInstallFailureCode::IntegrityMismatch,
    }
}

/// Outcome of an installation attempt after all evidence is collected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AttemptOutcome {
    /// Durable state to record.
    pub(super) state: RuntimeInstallState,
    /// Furthest stage actually reached.
    pub(super) stage: RuntimeInstallStage,
    /// Ownership to record; only a verified install may claim `GixGizManaged`.
    pub(super) ownership: RuntimeOwnership,
    /// Safe failure classification when the attempt failed.
    pub(super) failure: Option<RuntimeInstallFailureCode>,
    /// Reason the job needs a decision, when it does.
    pub(super) attention: Option<RuntimeInstallAttentionReason>,
    /// Safe recovery route when one exists.
    pub(super) recovery: Option<RuntimeInstallRecoveryAction>,
}

/// Resolves the outcome of a completed installation attempt.
///
/// `previous_ownership` is preserved on every non-success path so a failed or
/// uncertain attempt can never quietly adopt a runtime.
pub(super) fn resolve_attempt(
    execution: RuntimeInstallExecution,
    verification: RuntimeInstallVerificationResult,
    previous_ownership: RuntimeOwnership,
    retry_count: u32,
) -> AttemptOutcome {
    let retry_or_recreate = if retry_count < MAX_INSTALL_RETRIES {
        RuntimeInstallRecoveryAction::Retry
    } else {
        RuntimeInstallRecoveryAction::RecreatePlan
    };

    match execution {
        // The provider reported completion, but that alone proves nothing.
        // Ownership transfers only when every independent check passed.
        RuntimeInstallExecution::ReportedComplete if verification.is_verified() => AttemptOutcome {
            state: RuntimeInstallState::Ready,
            stage: RuntimeInstallStage::Ready,
            ownership: RuntimeOwnership::GixGizManaged,
            failure: None,
            attention: None,
            recovery: None,
        },
        RuntimeInstallExecution::ReportedComplete => AttemptOutcome {
            state: RuntimeInstallState::AttentionRequired,
            stage: RuntimeInstallStage::VerifyingRuntime,
            ownership: previous_ownership,
            failure: Some(classify_verification_gap(verification)),
            attention: Some(RuntimeInstallAttentionReason::RuntimeUnverified),
            recovery: Some(retry_or_recreate),
        },
        RuntimeInstallExecution::ReportedFailure => AttemptOutcome {
            state: RuntimeInstallState::Failed,
            stage: RuntimeInstallStage::Installing,
            ownership: previous_ownership,
            failure: Some(RuntimeInstallFailureCode::InstallerFailed),
            attention: None,
            recovery: Some(retry_or_recreate),
        },
        // An uncertain installer outcome must never be guessed in either
        // direction: it is neither success nor a clean failure.
        RuntimeInstallExecution::Uncertain => AttemptOutcome {
            state: RuntimeInstallState::AttentionRequired,
            stage: RuntimeInstallStage::Installing,
            ownership: previous_ownership,
            failure: None,
            attention: Some(RuntimeInstallAttentionReason::InstallOutcomeUncertain),
            recovery: Some(RuntimeInstallRecoveryAction::RecreatePlan),
        },
        // An execution result added by a newer adapter is not evidence of
        // success and must never claim ownership.
        _ => AttemptOutcome {
            state: RuntimeInstallState::AttentionRequired,
            stage: RuntimeInstallStage::Installing,
            ownership: previous_ownership,
            failure: None,
            attention: Some(RuntimeInstallAttentionReason::InstallOutcomeUncertain),
            recovery: Some(RuntimeInstallRecoveryAction::RecreatePlan),
        },
    }
}

/// Chooses the most specific failure for incomplete verification evidence.
fn classify_verification_gap(
    verification: RuntimeInstallVerificationResult,
) -> RuntimeInstallFailureCode {
    if !verification.executable_located {
        RuntimeInstallFailureCode::ExecutableNotFound
    } else if !verification.signature_valid || !verification.publisher_matched {
        RuntimeInstallFailureCode::ExecutableUntrusted
    } else if !verification.version_supported {
        RuntimeInstallFailureCode::VersionUnsupported
    } else {
        RuntimeInstallFailureCode::RuntimeUnhealthy
    }
}

/// Builds an honest effect report for an attempt that never executed anything.
pub(super) fn effects_before_execution(
    staged: RuntimeInstallEffectDisposition,
) -> RuntimeInstallEffectReport {
    RuntimeInstallEffectReport {
        effects: vec![
            RuntimeInstallEffect {
                kind: RuntimeInstallEffectKind::StagedInstallerArtifact,
                disposition: staged,
                description: "Installer artifact in GixGiz-owned staging.".to_owned(),
            },
            RuntimeInstallEffect {
                kind: RuntimeInstallEffectKind::PerUserApplicationFiles,
                disposition: RuntimeInstallEffectDisposition::NotStarted,
                description: "No application files were written.".to_owned(),
            },
            RuntimeInstallEffect {
                kind: RuntimeInstallEffectKind::OwnershipRecorded,
                disposition: RuntimeInstallEffectDisposition::NotStarted,
                description: "No ownership was recorded.".to_owned(),
            },
        ],
    }
}

/// Builds an honest effect report for an attempt that ran the installer.
pub(super) fn effects_after_execution(outcome: &AttemptOutcome) -> RuntimeInstallEffectReport {
    let (files, ownership) = match outcome.state {
        RuntimeInstallState::Ready => (
            RuntimeInstallEffectDisposition::Completed,
            RuntimeInstallEffectDisposition::Completed,
        ),
        // An uncertain installer leaves genuinely unknown filesystem effects.
        RuntimeInstallState::AttentionRequired
            if outcome.attention
                == Some(RuntimeInstallAttentionReason::InstallOutcomeUncertain) =>
        {
            (
                RuntimeInstallEffectDisposition::Uncertain,
                RuntimeInstallEffectDisposition::NotStarted,
            )
        }
        _ => (
            RuntimeInstallEffectDisposition::Uncertain,
            RuntimeInstallEffectDisposition::NotStarted,
        ),
    };

    RuntimeInstallEffectReport {
        effects: vec![
            RuntimeInstallEffect {
                kind: RuntimeInstallEffectKind::StagedInstallerArtifact,
                disposition: RuntimeInstallEffectDisposition::Retained,
                description: "Verified installer artifact retained for recovery.".to_owned(),
            },
            RuntimeInstallEffect {
                kind: RuntimeInstallEffectKind::PerUserApplicationFiles,
                disposition: files,
                description: "Application files written by the provider installer.".to_owned(),
            },
            RuntimeInstallEffect {
                kind: RuntimeInstallEffectKind::OwnershipRecorded,
                disposition: ownership,
                description: "GixGiz ownership record for the installed runtime.".to_owned(),
            },
        ],
    }
}

/// Reconciles a job interrupted by an earlier process exit.
///
/// Recovery never resumes a privileged or system-changing step. It re-detects
/// and reports honestly, so an interrupted attempt cannot silently complete or
/// silently repeat itself.
pub(super) fn reconcile_interrupted(
    verification: RuntimeInstallVerificationResult,
    previous_ownership: RuntimeOwnership,
) -> AttemptOutcome {
    if verification.is_verified() {
        // The installation demonstrably completed before the exit.
        return AttemptOutcome {
            state: RuntimeInstallState::Ready,
            stage: RuntimeInstallStage::Ready,
            ownership: RuntimeOwnership::GixGizManaged,
            failure: None,
            attention: None,
            recovery: None,
        };
    }
    AttemptOutcome {
        state: RuntimeInstallState::AttentionRequired,
        stage: RuntimeInstallStage::VerifyingRuntime,
        ownership: previous_ownership,
        failure: None,
        attention: Some(RuntimeInstallAttentionReason::InstallOutcomeUncertain),
        recovery: Some(RuntimeInstallRecoveryAction::RecreatePlan),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_verification() -> RuntimeInstallVerificationResult {
        RuntimeInstallVerificationResult {
            executable_located: true,
            signature_valid: true,
            publisher_matched: true,
            version_supported: true,
            endpoint_healthy: true,
        }
    }

    #[test]
    fn work_never_starts_without_an_approval_for_the_exact_revision() {
        assert_eq!(
            decide_start(RuntimeInstallState::Approved, 2, Some(2)),
            StartDecision::Admit
        );
        // A regenerated plan invalidates the earlier approval.
        assert_eq!(
            decide_start(RuntimeInstallState::Approved, 3, Some(2)),
            StartDecision::Reject(RuntimeInstallAttentionReason::ApprovalRequired)
        );
        assert_eq!(
            decide_start(RuntimeInstallState::AwaitingApproval, 1, None),
            StartDecision::Reject(RuntimeInstallAttentionReason::ApprovalRequired)
        );
        // A terminal or cancelled job cannot be restarted by an old approval.
        for state in [
            RuntimeInstallState::Cancelled,
            RuntimeInstallState::Ready,
            RuntimeInstallState::Failed,
            RuntimeInstallState::Running,
        ] {
            assert_eq!(
                decide_start(state, 1, Some(1)),
                StartDecision::Reject(RuntimeInstallAttentionReason::ApprovalRequired),
                "{state:?} must not start"
            );
        }
    }

    #[test]
    fn a_reported_completion_without_full_verification_never_becomes_ready() {
        // Each individually missing check must block Ready and keep ownership.
        type Gap = (
            fn(&mut RuntimeInstallVerificationResult),
            RuntimeInstallFailureCode,
        );
        let gaps: [Gap; 5] = [
            (
                |v| v.executable_located = false,
                RuntimeInstallFailureCode::ExecutableNotFound,
            ),
            (
                |v| v.signature_valid = false,
                RuntimeInstallFailureCode::ExecutableUntrusted,
            ),
            (
                |v| v.publisher_matched = false,
                RuntimeInstallFailureCode::ExecutableUntrusted,
            ),
            (
                |v| v.version_supported = false,
                RuntimeInstallFailureCode::VersionUnsupported,
            ),
            (
                |v| v.endpoint_healthy = false,
                RuntimeInstallFailureCode::RuntimeUnhealthy,
            ),
        ];

        for (break_check, expected) in gaps {
            let mut verification = full_verification();
            break_check(&mut verification);
            let outcome = resolve_attempt(
                RuntimeInstallExecution::ReportedComplete,
                verification,
                RuntimeOwnership::Unknown,
                0,
            );

            assert_eq!(outcome.state, RuntimeInstallState::AttentionRequired);
            assert_ne!(outcome.ownership, RuntimeOwnership::GixGizManaged);
            assert_eq!(outcome.failure, Some(expected));
        }
    }

    #[test]
    fn only_a_fully_verified_completion_claims_ownership() {
        let outcome = resolve_attempt(
            RuntimeInstallExecution::ReportedComplete,
            full_verification(),
            RuntimeOwnership::Unknown,
            0,
        );

        assert_eq!(outcome.state, RuntimeInstallState::Ready);
        assert_eq!(outcome.ownership, RuntimeOwnership::GixGizManaged);
        assert!(outcome.failure.is_none());
    }

    #[test]
    fn an_external_runtime_is_never_adopted_by_a_failed_attempt() {
        for execution in [
            RuntimeInstallExecution::ReportedFailure,
            RuntimeInstallExecution::Uncertain,
        ] {
            let outcome = resolve_attempt(
                execution,
                RuntimeInstallVerificationResult::default(),
                RuntimeOwnership::External,
                0,
            );

            assert_eq!(outcome.ownership, RuntimeOwnership::External);
            assert_ne!(outcome.state, RuntimeInstallState::Ready);
        }
    }

    #[test]
    fn an_uncertain_installer_is_neither_success_nor_clean_failure() {
        let outcome = resolve_attempt(
            RuntimeInstallExecution::Uncertain,
            full_verification(),
            RuntimeOwnership::Unknown,
            0,
        );

        assert_eq!(outcome.state, RuntimeInstallState::AttentionRequired);
        assert_eq!(
            outcome.attention,
            Some(RuntimeInstallAttentionReason::InstallOutcomeUncertain)
        );
        assert_eq!(
            outcome.recovery,
            Some(RuntimeInstallRecoveryAction::RecreatePlan)
        );
        assert_ne!(outcome.ownership, RuntimeOwnership::GixGizManaged);

        // Its filesystem effect is reported as genuinely unknown, not "none".
        let report = effects_after_execution(&outcome);
        let files = report
            .effects
            .iter()
            .find(|effect| effect.kind == RuntimeInstallEffectKind::PerUserApplicationFiles)
            .expect("files effect is declared");
        assert_eq!(
            files.disposition,
            RuntimeInstallEffectDisposition::Uncertain
        );
    }

    #[test]
    fn every_artifact_rejection_maps_to_a_distinct_safe_failure() {
        let cases = [
            (
                RuntimeArtifactRejection::InsufficientStagingSpace,
                RuntimeInstallFailureCode::InsufficientStorage,
            ),
            (
                RuntimeArtifactRejection::InsufficientInstallSpace,
                RuntimeInstallFailureCode::InsufficientStorage,
            ),
            (
                RuntimeArtifactRejection::SourceUntrusted,
                RuntimeInstallFailureCode::SourceUntrusted,
            ),
            (
                RuntimeArtifactRejection::TooLarge,
                RuntimeInstallFailureCode::ArtifactTooLarge,
            ),
            (
                RuntimeArtifactRejection::DigestMismatch,
                RuntimeInstallFailureCode::IntegrityMismatch,
            ),
            (
                RuntimeArtifactRejection::SignatureInvalid,
                RuntimeInstallFailureCode::SignatureInvalid,
            ),
            (
                RuntimeArtifactRejection::PublisherUnexpected,
                RuntimeInstallFailureCode::PublisherUnexpected,
            ),
        ];
        for (rejection, expected) in cases {
            assert_eq!(classify_rejection(rejection), expected);
        }
    }

    #[test]
    fn a_rejected_artifact_reports_no_application_or_ownership_effect() {
        let report = effects_before_execution(RuntimeInstallEffectDisposition::RolledBack);

        for effect in &report.effects {
            if effect.kind == RuntimeInstallEffectKind::StagedInstallerArtifact {
                assert_eq!(
                    effect.disposition,
                    RuntimeInstallEffectDisposition::RolledBack
                );
            } else {
                assert_eq!(
                    effect.disposition,
                    RuntimeInstallEffectDisposition::NotStarted,
                    "{:?} must not be claimed",
                    effect.kind
                );
            }
        }
    }

    #[test]
    fn retries_are_bounded_and_then_require_a_new_plan() {
        let failed = |retry| {
            resolve_attempt(
                RuntimeInstallExecution::ReportedFailure,
                RuntimeInstallVerificationResult::default(),
                RuntimeOwnership::Unknown,
                retry,
            )
            .recovery
        };

        assert_eq!(failed(0), Some(RuntimeInstallRecoveryAction::Retry));
        assert_eq!(
            failed(MAX_INSTALL_RETRIES - 1),
            Some(RuntimeInstallRecoveryAction::Retry)
        );
        assert_eq!(
            failed(MAX_INSTALL_RETRIES),
            Some(RuntimeInstallRecoveryAction::RecreatePlan)
        );
    }

    #[test]
    fn an_interrupted_job_recovers_only_through_fresh_verification() {
        // Verified after restart: the install really did complete.
        let recovered = reconcile_interrupted(full_verification(), RuntimeOwnership::Unknown);
        assert_eq!(recovered.state, RuntimeInstallState::Ready);
        assert_eq!(recovered.ownership, RuntimeOwnership::GixGizManaged);

        // Not verified: never a false Ready, and never a blind reinstall.
        let unresolved = reconcile_interrupted(
            RuntimeInstallVerificationResult::default(),
            RuntimeOwnership::External,
        );
        assert_eq!(unresolved.state, RuntimeInstallState::AttentionRequired);
        assert_eq!(unresolved.ownership, RuntimeOwnership::External);
        assert_eq!(
            unresolved.recovery,
            Some(RuntimeInstallRecoveryAction::RecreatePlan)
        );
    }
}
