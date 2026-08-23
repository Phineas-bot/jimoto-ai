//! Durable coordination for managed runtime installation.
//!
//! [`RuntimeInstallService`] owns the policy and is deliberately storage-free so
//! it stays testable. This coordinator is the only place that mirrors a job into
//! SQLite, which keeps the authoritative record in Rust and out of Flutter.

use std::sync::Arc;

use gixgiz_contracts::{
    RuntimeInstallApprovalDecision, RuntimeInstallApprovalRecord, RuntimeInstallAttentionReason,
    RuntimeInstallFailureCode, RuntimeInstallJobId, RuntimeInstallJobSnapshot,
    RuntimeInstallProgress, RuntimeInstallState, RuntimeOwnership, RuntimeState,
};
use gixgiz_persistence::{PersistedRuntimeInstallJob, Persistence, RuntimeInstallRepository};
use gixgiz_runtime::{
    INSTALL_PROGRESS_CHANNEL_CAPACITY, RuntimeInstaller, RuntimeOperationContext,
};
use tokio::sync::{Mutex, mpsc};

use super::{RuntimeInstallJob, RuntimeInstallService};
use crate::CoreError;

/// Durable, single-active runtime-installation coordinator.
pub struct RuntimeInstallCoordinator {
    service: RuntimeInstallService,
    repository: RuntimeInstallRepository,
    provider_id: String,
    /// Serializes approved system-changing work so one job runs at a time.
    active: Mutex<Option<RuntimeInstallJob>>,
    /// Snapshot readers observe.
    ///
    /// Installation work is long-running, so it must not hold the working lock
    /// while a status request is served. This is updated at each durable
    /// transition and is always the authoritative value already persisted.
    published: Mutex<Option<RuntimeInstallJobSnapshot>>,
}

impl RuntimeInstallCoordinator {
    /// Composes the coordinator over Rust-owned persistence.
    #[must_use]
    pub fn with_persistence(
        installer: Arc<dyn RuntimeInstaller>,
        persistence: &Persistence,
        provider_id: impl Into<String>,
    ) -> Self {
        Self {
            service: RuntimeInstallService::new(installer),
            repository: persistence.runtime_install(),
            provider_id: provider_id.into(),
            active: Mutex::new(None),
            published: Mutex::new(None),
        }
    }

    /// Builds a reviewable plan and persists it awaiting an explicit decision.
    pub async fn plan(
        &self,
        detected_state: RuntimeState,
        ownership: RuntimeOwnership,
        context: RuntimeOperationContext,
    ) -> Result<RuntimeInstallJobSnapshot, RuntimeInstallAttentionReason> {
        let job = self
            .service
            .plan(detected_state, ownership, context)
            .await?;
        self.persist(&job)
            .map_err(|_| RuntimeInstallAttentionReason::Unknown)?;
        let snapshot = job.snapshot.clone();
        *self.published.lock().await = Some(snapshot.clone());
        *self.active.lock().await = Some(job);
        Ok(snapshot)
    }

    /// Records an explicit decision for one exact plan revision.
    pub async fn approve(
        &self,
        record: RuntimeInstallApprovalRecord,
    ) -> Result<RuntimeInstallJobSnapshot, RuntimeInstallFailureCode> {
        let mut guard = self.active.lock().await;
        let job = guard
            .as_mut()
            .ok_or(RuntimeInstallFailureCode::OwnershipConflict)?;
        if job.snapshot.job_id != record.job_id {
            return Err(RuntimeInstallFailureCode::OwnershipConflict);
        }
        self.service.approve(job, record)?;
        self.persist(job)
            .map_err(|_| RuntimeInstallFailureCode::PersistenceUnavailable)?;
        let snapshot = job.snapshot.clone();
        *self.published.lock().await = Some(snapshot.clone());
        Ok(snapshot)
    }

    /// Admits approved installation work and runs it in the background.
    ///
    /// Installation takes far longer than one transport request, so this
    /// returns as soon as the job is durably `Running`. Callers observe the
    /// outcome through [`Self::status`], which reads persisted state rather
    /// than any client-side inference.
    pub async fn start(
        self: &Arc<Self>,
        job_id: RuntimeInstallJobId,
        progress: Option<mpsc::Sender<RuntimeInstallProgress>>,
        context: RuntimeOperationContext,
    ) -> Result<RuntimeInstallJobSnapshot, RuntimeInstallFailureCode> {
        let mut owned = {
            let mut guard = self.active.lock().await;
            match guard.as_ref() {
                Some(job) if job.snapshot.job_id == job_id => {}
                _ => return Err(RuntimeInstallFailureCode::OwnershipConflict),
            }
            guard
                .take()
                .ok_or(RuntimeInstallFailureCode::OwnershipConflict)?
        };

        // Refuse before any work when no approval binds the current revision.
        let approved = owned
            .snapshot
            .approval
            .as_ref()
            .map(|approval| approval.plan_revision);
        if approved != Some(owned.snapshot.plan.revision) {
            let snapshot = owned.snapshot.clone();
            *self.active.lock().await = Some(owned);
            *self.published.lock().await = Some(snapshot);
            return Err(RuntimeInstallFailureCode::OwnershipConflict);
        }

        owned.snapshot.state = RuntimeInstallState::Running;
        owned.snapshot.attention = None;
        if self.persist(&owned).is_err() {
            *self.active.lock().await = Some(owned);
            return Err(RuntimeInstallFailureCode::PersistenceUnavailable);
        }
        let admitted = owned.snapshot.clone();
        *self.published.lock().await = Some(admitted.clone());

        // Publish transfer progress as it arrives so a long download is visible
        // rather than opaque. Durable state still changes only at real
        // transitions; this updates the observable byte count only.
        let (sink, mut updates) = mpsc::channel(INSTALL_PROGRESS_CHANNEL_CAPACITY);
        let observer = Arc::clone(self);
        tokio::spawn(async move {
            while let Some(update) = updates.recv().await {
                {
                    let mut published = observer.published.lock().await;
                    if let Some(snapshot) = published.as_mut() {
                        snapshot.progress = Some(update);
                    }
                }
                if let Some(forward) = progress.as_ref() {
                    let _ = forward.send(update).await;
                }
            }
        });

        let coordinator = Arc::clone(self);
        tokio::spawn(async move {
            let mut job = owned;
            coordinator.service.run(&mut job, Some(sink), context).await;
            let _ = coordinator.persist(&job);
            *coordinator.published.lock().await = Some(job.snapshot.clone());
            *coordinator.active.lock().await = Some(job);
        });

        Ok(admitted)
    }

    /// Returns the authoritative published snapshot for one job.
    pub async fn status(&self, job_id: RuntimeInstallJobId) -> Option<RuntimeInstallJobSnapshot> {
        self.published
            .lock()
            .await
            .clone()
            .filter(|snapshot| snapshot.job_id == job_id)
    }

    /// Reconciles a job interrupted by an earlier process exit.
    ///
    /// Called during startup before the transport accepts requests, so a client
    /// can never observe a stale `Running` installation.
    pub async fn recover(
        &self,
        context: RuntimeOperationContext,
    ) -> Result<Option<RuntimeInstallJobSnapshot>, CoreError> {
        let provider_id = self.provider_id.clone();
        let repository = self.repository.clone();
        let Some(stored) = blocking_active(repository, provider_id).await? else {
            return Ok(None);
        };
        let mut guard = self.active.lock().await;
        let job = match guard.as_mut() {
            Some(job) if job.snapshot.job_id.to_string() == stored.job_id => job,
            _ => {
                // The durable record outlived the process. Mark it as needing a
                // decision rather than inventing a plan we can no longer show.
                let mut reconciled = stored.clone();
                reconciled.state = "attention_required".to_owned();
                reconciled.attention = Some("install_outcome_uncertain".to_owned());
                reconciled.recovery = Some("recreate_plan".to_owned());
                let repository = self.repository.clone();
                let record = reconciled;
                tokio::task::spawn_blocking(move || repository.upsert(&record))
                    .await
                    .map_err(|_| CoreError::SetupPersistenceUnavailable)?
                    .map_err(|_| CoreError::SetupPersistenceUnavailable)?;
                return Ok(None);
            }
        };
        self.service.recover(job, context).await;
        self.persist(job)
            .map_err(|_| CoreError::SetupPersistenceUnavailable)?;
        let snapshot = job.snapshot.clone();
        drop(guard);
        *self.published.lock().await = Some(snapshot.clone());
        Ok(Some(snapshot))
    }

    fn persist(&self, job: &RuntimeInstallJob) -> Result<(), CoreError> {
        let record = to_record(&job.snapshot, &self.provider_id);
        self.repository
            .upsert(&record)
            .map_err(|_| CoreError::SetupPersistenceUnavailable)
    }
}

async fn blocking_active(
    repository: RuntimeInstallRepository,
    provider_id: String,
) -> Result<Option<PersistedRuntimeInstallJob>, CoreError> {
    tokio::task::spawn_blocking(move || repository.active(&provider_id))
        .await
        .map_err(|_| CoreError::SetupPersistenceUnavailable)?
        .map_err(|_| CoreError::SetupPersistenceUnavailable)
}

fn to_record(
    snapshot: &RuntimeInstallJobSnapshot,
    provider_id: &str,
) -> PersistedRuntimeInstallJob {
    let component = snapshot.plan.components.first();
    PersistedRuntimeInstallJob {
        job_id: snapshot.job_id.to_string(),
        provider_id: provider_id.to_owned(),
        plan_revision: snapshot.plan.revision,
        version: component
            .map(|component| component.version.clone())
            .unwrap_or_else(|| "unknown".to_owned()),
        source_origin: component
            .map(|component| component.source_origin.clone())
            .unwrap_or_else(|| "unknown".to_owned()),
        artifact_name: component
            .map(|component| component.artifact_name.clone())
            .unwrap_or_else(|| "unknown".to_owned()),
        expected_size_bytes: snapshot.plan.expected_download_bytes,
        requires_administrator: snapshot.plan.requires_administrator,
        state: state_key(snapshot.state).to_owned(),
        stage: format!("{:?}", snapshot.stage).to_lowercase(),
        approved_revision: snapshot
            .approval
            .as_ref()
            .map(|approval| approval.plan_revision),
        approved_at_unix_ms: snapshot
            .approval
            .as_ref()
            .map(|approval| approval.decided_at_unix_ms as i64),
        transferred_bytes: snapshot.progress.map(|progress| progress.transferred_bytes),
        verification_json: snapshot
            .verification
            .and_then(|verification| serde_json::to_string(&verification).ok()),
        effects_json: serde_json::to_string(&snapshot.effects)
            .unwrap_or_else(|_| "{\"effects\":[]}".to_owned()),
        attention: snapshot
            .attention
            .map(|reason| format!("{reason:?}").to_lowercase()),
        failure: snapshot
            .failure
            .map(|failure| format!("{failure:?}").to_lowercase()),
        recovery: snapshot
            .recovery
            .map(|recovery| format!("{recovery:?}").to_lowercase()),
        ownership: ownership_key(snapshot.ownership).to_owned(),
        retry_count: snapshot.retry_count,
        created_at_unix_ms: snapshot.updated_at_unix_ms as i64,
        updated_at_unix_ms: snapshot.updated_at_unix_ms as i64,
    }
}

/// Maps a state to the exact token the SQLite constraint accepts.
const fn state_key(state: RuntimeInstallState) -> &'static str {
    match state {
        RuntimeInstallState::AwaitingApproval => "awaiting_approval",
        RuntimeInstallState::Approved => "approved",
        RuntimeInstallState::Running => "running",
        RuntimeInstallState::AttentionRequired => "attention_required",
        RuntimeInstallState::Ready => "ready",
        RuntimeInstallState::Failed => "failed",
        RuntimeInstallState::Cancelled => "cancelled",
        _ => "unknown",
    }
}

const fn ownership_key(ownership: RuntimeOwnership) -> &'static str {
    match ownership {
        RuntimeOwnership::External => "external",
        RuntimeOwnership::GixGizManaged => "gix_giz_managed",
        RuntimeOwnership::Bundled => "bundled",
        _ => "unknown",
    }
}

/// Returns whether a decision authorizes system-changing work.
#[must_use]
pub const fn decision_authorizes_work(decision: RuntimeInstallApprovalDecision) -> bool {
    matches!(decision, RuntimeInstallApprovalDecision::Approve)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_state_maps_to_an_accepted_storage_token() {
        // The SQLite CHECK constraint accepts exactly these tokens.
        let accepted = [
            "awaiting_approval",
            "approved",
            "running",
            "attention_required",
            "ready",
            "failed",
            "cancelled",
            "unknown",
        ];
        for state in [
            RuntimeInstallState::AwaitingApproval,
            RuntimeInstallState::Approved,
            RuntimeInstallState::Running,
            RuntimeInstallState::AttentionRequired,
            RuntimeInstallState::Ready,
            RuntimeInstallState::Failed,
            RuntimeInstallState::Cancelled,
            RuntimeInstallState::Unknown,
        ] {
            assert!(
                accepted.contains(&state_key(state)),
                "{state:?} maps to an unaccepted token"
            );
        }
    }

    #[test]
    fn every_ownership_maps_to_an_accepted_storage_token() {
        let accepted = ["external", "gix_giz_managed", "bundled", "unknown"];
        for ownership in [
            RuntimeOwnership::External,
            RuntimeOwnership::GixGizManaged,
            RuntimeOwnership::Bundled,
            RuntimeOwnership::Unknown,
        ] {
            assert!(accepted.contains(&ownership_key(ownership)));
        }
    }

    #[test]
    fn only_an_explicit_approval_authorizes_work() {
        assert!(decision_authorizes_work(
            RuntimeInstallApprovalDecision::Approve
        ));
        assert!(!decision_authorizes_work(
            RuntimeInstallApprovalDecision::Deny
        ));
        assert!(!decision_authorizes_work(
            RuntimeInstallApprovalDecision::Unknown
        ));
    }
}
