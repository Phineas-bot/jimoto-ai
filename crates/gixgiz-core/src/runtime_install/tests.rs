//! Deterministic coverage for approved managed runtime installation.
//!
//! These tests never download, elevate, execute an installer, or change Windows
//! installation state. The security-critical property under test throughout is
//! that an installer is executed only after an artifact verifies, and ownership
//! is recorded only after independent verification.

use std::sync::Arc;

use gixgiz_contracts::{
    CorrelationId, RequestId, RuntimeInstallApprovalDecision, RuntimeInstallApprovalRecord,
    RuntimeInstallAttentionReason, RuntimeInstallDestinationCategory, RuntimeInstallEffectKind,
    RuntimeInstallFailureCode, RuntimeInstallState, RuntimeInstallVerificationResult,
    RuntimeOwnership, RuntimeProviderId, RuntimeState,
};
use gixgiz_runtime::{
    FakeInstallCall, FakeRuntimeInstaller, RuntimeArtifactOutcome, RuntimeArtifactRejection,
    RuntimeInstallExecution, RuntimeInstallRefusal, RuntimeOperationContext, sample_candidate,
};

use super::{RuntimeInstallJob, RuntimeInstallService};

fn context() -> RuntimeOperationContext {
    RuntimeOperationContext::new(
        CorrelationId::new(),
        RequestId::new(),
        std::time::Duration::from_secs(5),
    )
}

fn service(installer: Arc<FakeRuntimeInstaller>) -> RuntimeInstallService {
    RuntimeInstallService::new(installer)
}

async fn planned(
    installer: Arc<FakeRuntimeInstaller>,
) -> (RuntimeInstallService, RuntimeInstallJob) {
    let service = service(installer);
    let job = service
        .plan(
            RuntimeState::NotInstalled,
            RuntimeOwnership::Unknown,
            context(),
        )
        .await
        .expect("a plan is offered when nothing is installed");
    (service, job)
}

fn approval(job: &RuntimeInstallJob) -> RuntimeInstallApprovalRecord {
    RuntimeInstallApprovalRecord {
        job_id: job.snapshot.job_id,
        plan_revision: job.snapshot.plan.revision,
        decision: RuntimeInstallApprovalDecision::Approve,
        provider_id: RuntimeProviderId::new("runtime"),
        version: job.snapshot.plan.components[0].version.clone(),
        authorized_effects: job.snapshot.plan.authorized_effects.clone(),
        requires_administrator: job.snapshot.plan.requires_administrator,
        correlation_id: CorrelationId::new(),
        request_id: RequestId::new(),
        decided_at_unix_ms: 1,
    }
}

#[tokio::test]
async fn an_existing_runtime_is_reused_and_never_replaced() {
    let installer = Arc::new(FakeRuntimeInstaller::succeeding());
    let service = service(installer.clone());

    // Every non-absent state must refuse to offer an installation, including an
    // incompatible one: replacing it needs a separate ownership decision.
    for state in [
        RuntimeState::Ready,
        RuntimeState::InstalledStopped,
        RuntimeState::Degraded,
        RuntimeState::Incompatible,
    ] {
        let refusal = service
            .plan(state, RuntimeOwnership::External, context())
            .await
            .expect_err("an installed runtime must not be replaced");

        assert_eq!(
            refusal,
            RuntimeInstallAttentionReason::ExternalRuntimePresent,
            "{state:?} must be reused, not overwritten"
        );
    }
    assert!(!installer.installer_was_executed());
}

#[tokio::test]
async fn planning_changes_nothing_and_awaits_an_explicit_decision() {
    let installer = Arc::new(FakeRuntimeInstaller::succeeding());
    let (_service, job) = planned(installer.clone()).await;

    assert_eq!(job.snapshot.state, RuntimeInstallState::AwaitingApproval);
    assert_eq!(
        job.snapshot.attention,
        Some(RuntimeInstallAttentionReason::ApprovalRequired)
    );
    assert_eq!(job.snapshot.ownership, RuntimeOwnership::Unknown);
    // Only the trusted candidate was consulted; nothing was transferred or run.
    assert_eq!(installer.calls(), vec![FakeInstallCall::Candidate]);
}

#[tokio::test]
async fn a_per_user_plan_declares_no_administrator_requirement() {
    let installer = Arc::new(FakeRuntimeInstaller::succeeding());
    let (_service, job) = planned(installer).await;

    assert!(!job.snapshot.plan.requires_administrator);
    assert_eq!(
        job.snapshot.plan.destination,
        RuntimeInstallDestinationCategory::PerUserApplicationDirectory
    );
    // Material effects outside the data root are still disclosed before approval.
    assert!(
        job.snapshot
            .plan
            .warnings
            .iter()
            .any(|warning| warning.contains("outside the GixGiz data folder"))
    );
    assert!(
        job.snapshot
            .plan
            .authorized_effects
            .contains(&RuntimeInstallEffectKind::PerUserApplicationFiles)
    );
}

#[tokio::test]
async fn nothing_is_downloaded_or_executed_without_approval() {
    let installer = Arc::new(FakeRuntimeInstaller::succeeding());
    let (service, mut job) = planned(installer.clone()).await;

    let state = service.run(&mut job, None, context()).await;

    assert_eq!(state, RuntimeInstallState::AttentionRequired);
    assert_eq!(
        job.snapshot.attention,
        Some(RuntimeInstallAttentionReason::ApprovalRequired)
    );
    assert!(!installer.installer_was_executed());
    assert!(!installer.calls().contains(&FakeInstallCall::StageArtifact));
}

#[tokio::test]
async fn denying_the_plan_records_a_cancelled_job_with_no_effects() {
    let installer = Arc::new(FakeRuntimeInstaller::succeeding());
    let (service, mut job) = planned(installer.clone()).await;

    let mut denial = approval(&job);
    denial.decision = RuntimeInstallApprovalDecision::Deny;
    service
        .approve(&mut job, denial)
        .expect("denial is recorded");

    assert_eq!(job.snapshot.state, RuntimeInstallState::Cancelled);
    assert!(job.snapshot.approval.is_none());
    assert!(!installer.installer_was_executed());

    // A denied job cannot then be started.
    let state = service.run(&mut job, None, context()).await;
    assert_eq!(state, RuntimeInstallState::AttentionRequired);
    assert!(!installer.installer_was_executed());
}

#[tokio::test]
async fn an_approval_for_a_different_revision_is_rejected() {
    let installer = Arc::new(FakeRuntimeInstaller::succeeding());
    let (service, mut job) = planned(installer).await;

    let mut stale = approval(&job);
    stale.plan_revision = job.snapshot.plan.revision + 1;

    assert_eq!(
        service.approve(&mut job, stale),
        Err(RuntimeInstallFailureCode::OwnershipConflict)
    );
    assert_eq!(job.snapshot.state, RuntimeInstallState::AwaitingApproval);
}

#[tokio::test]
async fn a_rejected_artifact_is_never_executed() {
    // This is the central security property of the whole workflow.
    let rejections = [
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
        (
            RuntimeArtifactRejection::SourceUntrusted,
            RuntimeInstallFailureCode::SourceUntrusted,
        ),
        (
            RuntimeArtifactRejection::TooLarge,
            RuntimeInstallFailureCode::ArtifactTooLarge,
        ),
    ];

    for (rejection, expected) in rejections {
        let installer = Arc::new(
            FakeRuntimeInstaller::succeeding()
                .with_artifact(RuntimeArtifactOutcome::Rejected { reason: rejection }),
        );
        let (service, mut job) = planned(installer.clone()).await;
        let record = approval(&job);
        service.approve(&mut job, record).expect("approved");

        let state = service.run(&mut job, None, context()).await;

        assert_eq!(state, RuntimeInstallState::Failed, "{rejection:?}");
        assert_eq!(job.snapshot.failure, Some(expected), "{rejection:?}");
        assert!(
            !installer.installer_was_executed(),
            "{rejection:?} must never reach the installer"
        );
        // GixGiz cleans up only its own staging.
        assert!(
            installer
                .calls()
                .contains(&FakeInstallCall::DiscardStagedArtifact)
        );
        assert_ne!(job.snapshot.ownership, RuntimeOwnership::GixGizManaged);
    }
}

#[tokio::test]
async fn insufficient_storage_stops_before_the_installer_runs() {
    // Discovering this mid-install wastes a large download and leaves the
    // destination in an uncertain state, so it must stop the attempt early.
    for reason in [
        RuntimeArtifactRejection::InsufficientStagingSpace,
        RuntimeArtifactRejection::InsufficientInstallSpace,
    ] {
        let installer = Arc::new(
            FakeRuntimeInstaller::succeeding()
                .with_artifact(RuntimeArtifactOutcome::Rejected { reason }),
        );
        let (service, mut job) = planned(installer.clone()).await;
        let record = approval(&job);
        service.approve(&mut job, record).expect("approved");

        let state = service.run(&mut job, None, context()).await;

        assert_eq!(state, RuntimeInstallState::Failed, "{reason:?}");
        assert_eq!(
            job.snapshot.failure,
            Some(RuntimeInstallFailureCode::InsufficientStorage),
            "{reason:?}"
        );
        assert!(
            !installer.installer_was_executed(),
            "{reason:?} must never reach the installer"
        );
        assert_ne!(job.snapshot.ownership, RuntimeOwnership::GixGizManaged);
    }
}

#[tokio::test]
async fn verification_always_precedes_execution() {
    let installer = Arc::new(FakeRuntimeInstaller::succeeding());
    let (service, mut job) = planned(installer.clone()).await;
    let record = approval(&job);
    service.approve(&mut job, record).expect("approved");

    service.run(&mut job, None, context()).await;

    let calls = installer.calls();
    let staged = calls
        .iter()
        .position(|call| *call == FakeInstallCall::StageArtifact)
        .expect("artifact was staged");
    let ran = calls
        .iter()
        .position(|call| *call == FakeInstallCall::RunInstaller)
        .expect("installer ran");
    assert!(
        staged < ran,
        "staging and verification must precede execution"
    );
}

#[tokio::test]
async fn a_fully_verified_installation_becomes_ready_and_owned() {
    let installer = Arc::new(FakeRuntimeInstaller::succeeding());
    let (service, mut job) = planned(installer).await;
    let record = approval(&job);
    service.approve(&mut job, record).expect("approved");

    let state = service.run(&mut job, None, context()).await;

    assert_eq!(state, RuntimeInstallState::Ready);
    assert_eq!(job.snapshot.ownership, RuntimeOwnership::GixGizManaged);
    assert!(job.snapshot.verification.expect("evidence").is_verified());
    assert!(job.snapshot.failure.is_none());
}

#[tokio::test]
async fn a_successful_installer_with_unhealthy_runtime_never_becomes_ready() {
    let installer = Arc::new(FakeRuntimeInstaller::succeeding().with_verification(
        RuntimeInstallVerificationResult {
            executable_located: true,
            signature_valid: true,
            publisher_matched: true,
            version_supported: true,
            endpoint_healthy: false,
        },
    ));
    let (service, mut job) = planned(installer).await;
    let record = approval(&job);
    service.approve(&mut job, record).expect("approved");

    let state = service.run(&mut job, None, context()).await;

    assert_eq!(state, RuntimeInstallState::AttentionRequired);
    assert_eq!(
        job.snapshot.failure,
        Some(RuntimeInstallFailureCode::RuntimeUnhealthy)
    );
    assert_ne!(job.snapshot.ownership, RuntimeOwnership::GixGizManaged);
}

#[tokio::test]
async fn an_untrusted_installed_executable_never_claims_ownership() {
    let installer = Arc::new(FakeRuntimeInstaller::succeeding().with_verification(
        RuntimeInstallVerificationResult {
            executable_located: true,
            signature_valid: false,
            publisher_matched: false,
            version_supported: true,
            endpoint_healthy: true,
        },
    ));
    let (service, mut job) = planned(installer).await;
    let record = approval(&job);
    service.approve(&mut job, record).expect("approved");

    service.run(&mut job, None, context()).await;

    assert_eq!(
        job.snapshot.failure,
        Some(RuntimeInstallFailureCode::ExecutableUntrusted)
    );
    assert_ne!(job.snapshot.ownership, RuntimeOwnership::GixGizManaged);
}

#[tokio::test]
async fn an_installer_failure_is_terminal_and_owns_nothing() {
    let installer = Arc::new(
        FakeRuntimeInstaller::succeeding()
            .with_execution(RuntimeInstallExecution::ReportedFailure)
            .with_verification(RuntimeInstallVerificationResult::default()),
    );
    let (service, mut job) = planned(installer).await;
    let record = approval(&job);
    service.approve(&mut job, record).expect("approved");

    let state = service.run(&mut job, None, context()).await;

    assert_eq!(state, RuntimeInstallState::Failed);
    assert_eq!(
        job.snapshot.failure,
        Some(RuntimeInstallFailureCode::InstallerFailed)
    );
    assert_ne!(job.snapshot.ownership, RuntimeOwnership::GixGizManaged);
}

#[tokio::test]
async fn an_uncertain_installer_outcome_is_reported_honestly() {
    let installer = Arc::new(
        FakeRuntimeInstaller::succeeding().with_execution(RuntimeInstallExecution::Uncertain),
    );
    let (service, mut job) = planned(installer).await;
    let record = approval(&job);
    service.approve(&mut job, record).expect("approved");

    let state = service.run(&mut job, None, context()).await;

    assert_eq!(state, RuntimeInstallState::AttentionRequired);
    assert_eq!(
        job.snapshot.attention,
        Some(RuntimeInstallAttentionReason::InstallOutcomeUncertain)
    );
    assert_ne!(job.snapshot.ownership, RuntimeOwnership::GixGizManaged);
}

#[tokio::test]
async fn a_refused_candidate_never_reaches_staging() {
    for refusal in [
        RuntimeInstallRefusal::NoIntegrityEvidence,
        RuntimeInstallRefusal::NoTrustedArtifact,
        RuntimeInstallRefusal::Unsupported,
    ] {
        let installer = Arc::new(FakeRuntimeInstaller::succeeding().with_candidate(Err(refusal)));
        let service = service(installer.clone());

        let error = service
            .plan(
                RuntimeState::NotInstalled,
                RuntimeOwnership::Unknown,
                context(),
            )
            .await
            .expect_err("a refused candidate offers no plan");

        assert!(matches!(
            error,
            RuntimeInstallAttentionReason::ArtifactRejected
                | RuntimeInstallAttentionReason::Unknown
                | RuntimeInstallAttentionReason::ExternalRuntimePresent
        ));
        assert!(!installer.calls().contains(&FakeInstallCall::StageArtifact));
        assert!(!installer.installer_was_executed());
    }
}

#[tokio::test]
async fn an_interrupted_job_recovers_through_detection_without_reinstalling() {
    let installer = Arc::new(
        FakeRuntimeInstaller::succeeding()
            .with_verification(RuntimeInstallVerificationResult::default()),
    );
    let (service, mut job) = planned(installer.clone()).await;
    let record = approval(&job);
    service.approve(&mut job, record).expect("approved");
    job.snapshot.state = RuntimeInstallState::Running;

    let state = service.recover(&mut job, context()).await;

    assert_eq!(state, RuntimeInstallState::AttentionRequired);
    assert_ne!(job.snapshot.ownership, RuntimeOwnership::GixGizManaged);
    // Recovery re-detects; it must never blindly run the installer again.
    assert!(!installer.installer_was_executed());
}

#[tokio::test]
async fn recovery_confirms_a_genuinely_completed_installation() {
    let installer = Arc::new(FakeRuntimeInstaller::succeeding());
    let (service, mut job) = planned(installer.clone()).await;
    let record = approval(&job);
    service.approve(&mut job, record).expect("approved");
    job.snapshot.state = RuntimeInstallState::Running;

    let state = service.recover(&mut job, context()).await;

    assert_eq!(state, RuntimeInstallState::Ready);
    assert_eq!(job.snapshot.ownership, RuntimeOwnership::GixGizManaged);
    assert!(!installer.installer_was_executed());
}

#[tokio::test]
async fn bounded_transfer_progress_is_published() {
    let installer = Arc::new(FakeRuntimeInstaller::succeeding());
    let (service, mut job) = planned(installer).await;
    let record = approval(&job);
    service.approve(&mut job, record).expect("approved");

    let (sender, mut receiver) = tokio::sync::mpsc::channel(8);
    service.run(&mut job, Some(sender), context()).await;

    let update = receiver.recv().await.expect("progress is observed");
    assert_eq!(update.transferred_bytes, 1024);
    assert_eq!(
        update.expected_bytes,
        Some(sample_candidate().expected_size_bytes)
    );
}

/// Manual end-to-end drive of the real managed installation.
///
/// Downloads the pinned installer, verifies it, runs it, and verifies the
/// installed runtime. This changes the machine and is never run by CI.
#[tokio::test]
#[ignore = "manual: really installs the pinned runtime on this machine"]
async fn manual_real_runtime_installation() {
    use gixgiz_runtime::RuntimeDetector;
    use gixgiz_runtime_ollama::{OllamaAdapter, OllamaInstaller};
    use std::time::Duration;

    let staging = std::path::PathBuf::from(
        std::env::var("GIXGIZ_STAGING").expect("set GIXGIZ_STAGING to a staging directory"),
    );
    std::fs::create_dir_all(&staging).expect("staging exists");

    let adapter = std::sync::Arc::new(OllamaAdapter::for_current_user().expect("adapter composes"));
    let installer = std::sync::Arc::new(OllamaInstaller::for_current_user(
        staging,
        None,
        adapter.clone(),
    ));
    let service = RuntimeInstallService::new(installer);

    let long = RuntimeOperationContext::new(
        CorrelationId::new(),
        RequestId::new(),
        Duration::from_secs(2 * 60 * 60),
    );

    println!("--- detecting ---");
    let observation = adapter
        .detect(RuntimeOperationContext::new(
            CorrelationId::new(),
            RequestId::new(),
            Duration::from_secs(30),
        ))
        .await
        .expect("detection succeeds");
    println!("state: {:?}", observation.state);

    println!("--- planning ---");
    let mut job = match service
        .plan(observation.state, RuntimeOwnership::Unknown, long.clone())
        .await
    {
        Ok(job) => job,
        Err(reason) => panic!("no plan offered: {reason:?}"),
    };
    let component = &job.snapshot.plan.components[0];
    println!("version   : {}", component.version);
    println!("origin    : {}", component.source_origin);
    println!("artifact  : {}", component.artifact_name);
    println!("publisher : {}", component.expected_publisher);
    println!("admin     : {}", job.snapshot.plan.requires_administrator);

    println!("--- approving ---");
    let record = RuntimeInstallApprovalRecord {
        job_id: job.snapshot.job_id,
        plan_revision: job.snapshot.plan.revision,
        decision: RuntimeInstallApprovalDecision::Approve,
        provider_id: job.snapshot.plan.provider_id.clone(),
        version: component.version.clone(),
        authorized_effects: job.snapshot.plan.authorized_effects.clone(),
        requires_administrator: job.snapshot.plan.requires_administrator,
        correlation_id: CorrelationId::new(),
        request_id: RequestId::new(),
        decided_at_unix_ms: 1,
    };
    service
        .approve(&mut job, record)
        .expect("approval recorded");

    println!("--- running (this downloads and installs) ---");
    let (sink, mut updates) =
        tokio::sync::mpsc::channel::<gixgiz_contracts::RuntimeInstallProgress>(16);
    let reporter = tokio::spawn(async move {
        while let Some(update) = updates.recv().await {
            let total = update.expected_bytes.unwrap_or(0);
            let percent = if total > 0 {
                (update.transferred_bytes as f64 / total as f64 * 100.0).round()
            } else {
                0.0
            };
            println!(
                "  progress {:>6} MB / {:>6} MB  ({percent:.0}%)",
                update.transferred_bytes / (1024 * 1024),
                total / (1024 * 1024),
            );
        }
    });

    let state = service.run(&mut job, Some(sink), long).await;
    let _ = reporter.await;

    println!("--- result ---");
    println!("state        : {state:?}");
    println!("stage        : {:?}", job.snapshot.stage);
    println!("ownership    : {:?}", job.snapshot.ownership);
    println!("failure      : {:?}", job.snapshot.failure);
    println!("attention    : {:?}", job.snapshot.attention);
    if let Some(verification) = job.snapshot.verification {
        println!("verification : {verification:?}");
    }
    for effect in &job.snapshot.effects.effects {
        println!("effect {:?} -> {:?}", effect.kind, effect.disposition);
    }
}
