//! Core orchestration for approved managed runtime installation.
//!
//! This module owns *policy*: whether an installation may be offered, whether
//! approved work may begin, and what observed evidence is allowed to mean. The
//! adapter owns the provider mechanics behind [`RuntimeInstaller`].
//!
//! Ordering is deliberate and enforced here rather than in the adapter:
//!
//! ```text
//! detect -> offer plan -> approve exact revision -> stage -> VERIFY
//!        -> execute (only when verified) -> verify install -> own
//! ```

mod coordinator;
mod state_machine;

use std::sync::Arc;

use gixgiz_contracts::{
    RUNTIME_INSTALL_SCHEMA_VERSION, RuntimeInstallApprovalDecision, RuntimeInstallApprovalRecord,
    RuntimeInstallAttentionReason, RuntimeInstallComponent, RuntimeInstallEffectKind,
    RuntimeInstallEffectReport, RuntimeInstallFailureCode, RuntimeInstallJobId,
    RuntimeInstallJobSnapshot, RuntimeInstallPlan, RuntimeInstallProgress, RuntimeInstallStage,
    RuntimeInstallState, RuntimeInstallVerificationResult, RuntimeOwnership, RuntimeState,
};
use gixgiz_runtime::{
    INSTALL_PROGRESS_CHANNEL_CAPACITY, RuntimeArtifactOutcome, RuntimeInstallCandidate,
    RuntimeInstallRefusal, RuntimeInstaller, RuntimeOperationContext,
};
use tokio::sync::mpsc;

use state_machine::{
    AttemptOutcome, StartDecision, decide_start, effects_after_execution, effects_before_execution,
    reconcile_interrupted, resolve_attempt,
};

pub use coordinator::{RuntimeInstallCoordinator, decision_authorizes_work};
pub use state_machine::MAX_INSTALL_RETRIES;

use gixgiz_contracts::RuntimeInstallEffectDisposition;

/// Durable installation job owned by core.
///
/// Held in memory by this service and mirrored to SQLite by the caller. The
/// separation keeps the policy testable without a database.
#[derive(Clone, Debug)]
pub struct RuntimeInstallJob {
    /// Current authoritative snapshot.
    pub snapshot: RuntimeInstallJobSnapshot,
}

/// Orchestrates approved managed runtime installation.
pub struct RuntimeInstallService {
    installer: Arc<dyn RuntimeInstaller>,
}

impl RuntimeInstallService {
    /// Composes the service over one provider installer.
    #[must_use]
    pub fn new(installer: Arc<dyn RuntimeInstaller>) -> Self {
        Self { installer }
    }

    /// Builds a reviewable plan, or explains why no installation is offered.
    ///
    /// Performs no download, execution, or system change. An already-present
    /// runtime that GixGiz does not own is never replaced: the caller is told
    /// to reuse it instead.
    pub async fn plan(
        &self,
        detected_state: RuntimeState,
        ownership: RuntimeOwnership,
        context: RuntimeOperationContext,
    ) -> Result<RuntimeInstallJob, RuntimeInstallAttentionReason> {
        // Detection comes first: an existing installation is reused, never
        // overwritten, regardless of whether its version is supported.
        if detected_state != RuntimeState::NotInstalled {
            return Err(RuntimeInstallAttentionReason::ExternalRuntimePresent);
        }
        if ownership == RuntimeOwnership::External {
            return Err(RuntimeInstallAttentionReason::ExternalRuntimePresent);
        }

        let candidate = self
            .installer
            .candidate(context)
            .await
            .map_err(|_| RuntimeInstallAttentionReason::Unknown)?
            .map_err(refusal_attention)?;

        Ok(RuntimeInstallJob {
            snapshot: new_snapshot(plan_from_candidate(&candidate), ownership),
        })
    }

    /// Records an explicit decision for one exact plan revision.
    pub fn approve(
        &self,
        job: &mut RuntimeInstallJob,
        record: RuntimeInstallApprovalRecord,
    ) -> Result<(), RuntimeInstallFailureCode> {
        if record.plan_revision != job.snapshot.plan.revision {
            return Err(RuntimeInstallFailureCode::OwnershipConflict);
        }
        match record.decision {
            RuntimeInstallApprovalDecision::Approve => {
                job.snapshot.state = RuntimeInstallState::Approved;
                job.snapshot.stage = RuntimeInstallStage::Approved;
                job.snapshot.attention = None;
                job.snapshot.approval = Some(record);
            }
            // Anything that is not an explicit approval is treated as a denial.
            _ => {
                job.snapshot.state = RuntimeInstallState::Cancelled;
                job.snapshot.stage = RuntimeInstallStage::AwaitingApproval;
                job.snapshot.attention = None;
                job.snapshot.approval = None;
                job.snapshot.effects =
                    effects_before_execution(RuntimeInstallEffectDisposition::NotStarted);
            }
        }
        Ok(())
    }

    /// Runs one approved installation attempt.
    ///
    /// Returns without touching the system when no approval binds the current
    /// plan revision. Never executes an installer whose artifact verification
    /// did not pass.
    pub async fn run(
        &self,
        job: &mut RuntimeInstallJob,
        progress: Option<mpsc::Sender<RuntimeInstallProgress>>,
        context: RuntimeOperationContext,
    ) -> RuntimeInstallState {
        let approved_revision = job
            .snapshot
            .approval
            .as_ref()
            .map(|approval| approval.plan_revision);
        if let StartDecision::Reject(reason) = decide_start(
            job.snapshot.state,
            job.snapshot.plan.revision,
            approved_revision,
        ) {
            job.snapshot.state = RuntimeInstallState::AttentionRequired;
            job.snapshot.attention = Some(reason);
            job.snapshot.effects =
                effects_before_execution(RuntimeInstallEffectDisposition::NotStarted);
            return job.snapshot.state;
        }

        let candidate = match self.installer.candidate(context.clone()).await {
            Ok(Ok(candidate)) => candidate,
            Ok(Err(refusal)) => {
                return self.stop_before_execution(
                    job,
                    Some(refusal_attention(refusal)),
                    None,
                    RuntimeInstallStage::PreparingPlan,
                );
            }
            Err(_) => {
                return self.stop_before_execution(
                    job,
                    None,
                    Some(RuntimeInstallFailureCode::DownloadFailed),
                    RuntimeInstallStage::PreparingPlan,
                );
            }
        };

        job.snapshot.state = RuntimeInstallState::Running;
        job.snapshot.stage = RuntimeInstallStage::Downloading;

        let (sender, mut receiver) = mpsc::channel(INSTALL_PROGRESS_CHANNEL_CAPACITY);
        let forward = tokio::spawn(async move {
            let mut latest = None;
            while let Some(update) = receiver.recv().await {
                latest = Some(update);
                if let Some(sink) = progress.as_ref() {
                    let _ = sink.send(update).await;
                }
            }
            latest
        });

        let staged = self
            .installer
            .stage_artifact(&candidate, sender, context.clone())
            .await;
        job.snapshot.progress = forward.await.ok().flatten();
        job.snapshot.stage = RuntimeInstallStage::VerifyingArtifact;

        match staged {
            Ok(RuntimeArtifactOutcome::Verified { .. }) => {}
            Ok(RuntimeArtifactOutcome::Rejected { reason }) => {
                // The artifact never becomes executable. Clean up our own
                // staging and stop; nothing outside GixGiz was touched.
                let _ = self
                    .installer
                    .discard_staged_artifact(&candidate, context)
                    .await;
                job.snapshot.effects =
                    effects_before_execution(RuntimeInstallEffectDisposition::RolledBack);
                job.snapshot.state = RuntimeInstallState::Failed;
                job.snapshot.stage = RuntimeInstallStage::VerifyingArtifact;
                job.snapshot.failure = Some(state_machine::classify_rejection(reason));
                job.snapshot.attention = Some(RuntimeInstallAttentionReason::ArtifactRejected);
                return job.snapshot.state;
            }
            Err(_) => {
                return self.stop_before_execution(
                    job,
                    None,
                    Some(RuntimeInstallFailureCode::DownloadFailed),
                    RuntimeInstallStage::Downloading,
                );
            }
            // An outcome kind added by a newer adapter is not evidence that the
            // artifact is safe to execute, so it stops here.
            Ok(_) => {
                let _ = self
                    .installer
                    .discard_staged_artifact(&candidate, context)
                    .await;
                return self.stop_before_execution(
                    job,
                    Some(RuntimeInstallAttentionReason::ArtifactRejected),
                    Some(RuntimeInstallFailureCode::IntegrityMismatch),
                    RuntimeInstallStage::VerifyingArtifact,
                );
            }
        }

        job.snapshot.stage = RuntimeInstallStage::Installing;
        let execution = match self
            .installer
            .run_installer(&candidate, context.clone())
            .await
        {
            Ok(execution) => execution,
            Err(_) => gixgiz_runtime::RuntimeInstallExecution::Uncertain,
        };

        job.snapshot.stage = RuntimeInstallStage::VerifyingExecutable;
        let verification = self
            .installer
            .verify_installation(&candidate, context)
            .await
            .unwrap_or_default();

        let outcome = resolve_attempt(
            execution,
            verification,
            job.snapshot.ownership,
            job.snapshot.retry_count,
        );
        self.apply(job, &outcome, Some(verification));
        job.snapshot.effects = effects_after_execution(&outcome);
        job.snapshot.state
    }

    /// Reconciles a job interrupted by an earlier process exit.
    ///
    /// Re-detects rather than resuming, so an interrupted attempt can never
    /// complete silently or repeat a system change blindly.
    pub async fn recover(
        &self,
        job: &mut RuntimeInstallJob,
        context: RuntimeOperationContext,
    ) -> RuntimeInstallState {
        if job.snapshot.state.is_terminal() {
            return job.snapshot.state;
        }
        let candidate = match self.installer.candidate(context.clone()).await {
            Ok(Ok(candidate)) => candidate,
            _ => {
                job.snapshot.state = RuntimeInstallState::AttentionRequired;
                job.snapshot.attention =
                    Some(RuntimeInstallAttentionReason::InstallOutcomeUncertain);
                return job.snapshot.state;
            }
        };
        let verification = self
            .installer
            .verify_installation(&candidate, context)
            .await
            .unwrap_or_default();
        let outcome = reconcile_interrupted(verification, job.snapshot.ownership);
        self.apply(job, &outcome, Some(verification));
        job.snapshot.state
    }

    fn stop_before_execution(
        &self,
        job: &mut RuntimeInstallJob,
        attention: Option<RuntimeInstallAttentionReason>,
        failure: Option<RuntimeInstallFailureCode>,
        stage: RuntimeInstallStage,
    ) -> RuntimeInstallState {
        job.snapshot.state = if failure.is_some() {
            RuntimeInstallState::Failed
        } else {
            RuntimeInstallState::AttentionRequired
        };
        job.snapshot.stage = stage;
        job.snapshot.attention = attention;
        job.snapshot.failure = failure;
        job.snapshot.effects =
            effects_before_execution(RuntimeInstallEffectDisposition::NotStarted);
        job.snapshot.state
    }

    fn apply(
        &self,
        job: &mut RuntimeInstallJob,
        outcome: &AttemptOutcome,
        verification: Option<RuntimeInstallVerificationResult>,
    ) {
        job.snapshot.state = outcome.state;
        job.snapshot.stage = outcome.stage;
        job.snapshot.ownership = outcome.ownership;
        job.snapshot.failure = outcome.failure;
        job.snapshot.attention = outcome.attention;
        job.snapshot.recovery = outcome.recovery;
        job.snapshot.verification = verification;
        if outcome.state != RuntimeInstallState::Ready {
            job.snapshot.retry_count = job.snapshot.retry_count.saturating_add(1);
        }
    }
}

fn refusal_attention(refusal: RuntimeInstallRefusal) -> RuntimeInstallAttentionReason {
    match refusal {
        RuntimeInstallRefusal::AlreadyInstalled => {
            RuntimeInstallAttentionReason::ExternalRuntimePresent
        }
        RuntimeInstallRefusal::NoIntegrityEvidence => {
            RuntimeInstallAttentionReason::ArtifactRejected
        }
        // An unrecognized refusal is still a refusal: no plan is offered.
        _ => RuntimeInstallAttentionReason::Unknown,
    }
}

fn plan_from_candidate(candidate: &RuntimeInstallCandidate) -> RuntimeInstallPlan {
    let mut warnings = vec![
        "Installing writes files outside the GixGiz data folder.".to_owned(),
        "Removing GixGiz does not remove this runtime.".to_owned(),
    ];
    if candidate.requires_administrator {
        warnings.push("Windows will ask for administrator permission.".to_owned());
    }

    RuntimeInstallPlan {
        schema_version: RUNTIME_INSTALL_SCHEMA_VERSION,
        job_id: RuntimeInstallJobId::new(),
        revision: 1,
        provider_id: gixgiz_contracts::RuntimeProviderId::new("runtime"),
        runtime_display_name: gixgiz_contracts::RuntimeDisplayName::new("Local AI runtime"),
        components: vec![RuntimeInstallComponent {
            display_name: "Local AI runtime".to_owned(),
            version: candidate.version.clone(),
            source_origin: candidate.source_origin.clone(),
            artifact_name: candidate.artifact_name.clone(),
            expected_size_bytes: candidate.expected_size_bytes,
            integrity_evidence: candidate.integrity_evidence,
            expected_publisher: candidate.expected_publisher.clone(),
        }],
        destination: candidate.destination,
        expected_download_bytes: candidate.expected_size_bytes,
        requires_administrator: candidate.requires_administrator,
        ownership_after_success: RuntimeOwnership::GixGizManaged,
        authorized_effects: vec![
            RuntimeInstallEffectKind::StagedInstallerArtifact,
            RuntimeInstallEffectKind::PerUserApplicationFiles,
            RuntimeInstallEffectKind::PerUserUninstallRegistration,
            RuntimeInstallEffectKind::OwnershipRecorded,
        ],
        reasons: vec!["No local AI runtime was found on this PC.".to_owned()],
        warnings,
    }
}

fn new_snapshot(
    plan: RuntimeInstallPlan,
    ownership: RuntimeOwnership,
) -> RuntimeInstallJobSnapshot {
    RuntimeInstallJobSnapshot {
        schema_version: RUNTIME_INSTALL_SCHEMA_VERSION,
        job_id: plan.job_id,
        state: RuntimeInstallState::AwaitingApproval,
        stage: RuntimeInstallStage::AwaitingApproval,
        plan,
        approval: None,
        progress: None,
        verification: None,
        effects: RuntimeInstallEffectReport::default(),
        attention: Some(RuntimeInstallAttentionReason::ApprovalRequired),
        failure: None,
        recovery: None,
        ownership,
        retry_count: 0,
        updated_at_unix_ms: 0,
    }
}

#[cfg(test)]
mod tests;
