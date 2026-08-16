//! Durable provider-neutral model setup orchestration.

mod state_machine;

use std::{
    collections::{HashMap, hash_map::Entry},
    str::FromStr,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use gixgiz_contracts::{
    CorrelationId, ErrorCategory, MemoryEstimate, ModelAcquisitionPhase, ModelAcquisitionProgress,
    ModelIntegrityState, ModelLifecycleState, ModelMetadata, ModelProviderArtifact,
    ModelVerificationResult, ModelVerificationState, ProviderRegistrationState, RecoveryAction,
    RecoveryGuidance, RequestId, ResourceEstimate, RuntimeConsentState, RuntimeOwnership,
    RuntimeState, SETUP_WORKFLOW_SCHEMA_VERSION, SafeErrorPayload, SetupApprovalDecision,
    SetupApprovalRecord, SetupApprovalRequest, SetupApprovalResponse, SetupAttentionReason,
    SetupCancellationReport, SetupDestinationCategory, SetupEffect, SetupEffectDisposition,
    SetupEffectKind, SetupEffectReport, SetupJobCancelRequest, SetupJobCancelResponse,
    SetupJobEvent, SetupJobEventKind, SetupJobEventsRequest, SetupJobEventsResponse, SetupJobId,
    SetupJobRecoveryRequest, SetupJobRecoveryResponse, SetupJobRetryRequest, SetupJobRetryResponse,
    SetupJobSnapshot, SetupJobStartRequest, SetupJobStartResponse, SetupJobState,
    SetupJobStatusRequest, SetupJobStatusResponse, SetupJobTerminalState, SetupPlan,
    SetupPlanComponent, SetupPlanComponentKind, SetupPlanRequest, SetupPlanResponse, SetupReason,
    SetupReasonCode, SetupRecoveryAction, SetupStage, SetupWarning, SetupWarningCode,
    StorageEstimate,
};
use gixgiz_persistence::{
    JobId, PersistedArtifactUpdate, PersistedDestinationCategory, PersistedEffectDisposition,
    PersistedModelArtifact, PersistedModelIntegrity, PersistedModelLifecycle,
    PersistedModelVerification, PersistedSetupApproval, PersistedSetupApprovalDecision,
    PersistedSetupApprovalInput, PersistedSetupEffect, PersistedSetupEffectInput,
    PersistedSetupEvent, PersistedSetupEventKind, PersistedSetupJob, PersistedSetupNotice,
    PersistedSetupPlan, PersistedSetupProgress, PersistedSetupState, PersistedSetupTransition,
    Persistence, PersistenceError, SetupJobRepository,
};
use gixgiz_runtime::{
    MODEL_PROGRESS_CHANNEL_CAPACITY, RuntimeCancellationSemantics, RuntimeCancellationToken,
    RuntimeError, RuntimeModelAcquisitionPlan, RuntimeModelAcquisitionResult,
    RuntimeModelAcquisitionStatus, RuntimeOperationContext, RuntimeProvider,
    RuntimeStorageAvailability,
};

use crate::{CapabilityEngine, CoreError, RuntimeService};
use state_machine::{stage_for, validate_transition};

const PREPARATION_TIMEOUT: Duration = Duration::from_secs(30);
const MODEL_INSPECTION_TIMEOUT: Duration = Duration::from_secs(60);
const READINESS_TIMEOUT: Duration = Duration::from_secs(180);
const ACQUISITION_TIMEOUT: Duration = Duration::from_secs(6 * 60 * 60);
const STORAGE_CHECKPOINT_BYTES: u64 = 512 * 1024 * 1024;
const STORAGE_CHECKPOINT_BASIS_POINTS: u16 = 2_500;
const STORAGE_CHECKPOINT_INTERVAL: Duration = Duration::from_secs(30);
const PERSISTENCE_CONTENTION_RETRIES: usize = 50;
const PERSISTENCE_CONTENTION_RETRY_DELAY: Duration = Duration::from_millis(2);
const MAX_SETUP_RETRIES: u32 = 8;
const EVENT_PAGE_MAX: u32 = 256;
const SAFE_PROVIDER_SUMMARY_BYTES_MAX: usize = 256;

const EFFECT_ACQUISITION: &str = "provider_model_acquisition";
const EFFECT_REGISTRATION: &str = "provider_model_registration";
const EFFECT_INFERENCE: &str = "readiness_inference";
const EFFECT_METADATA: &str = "metadata_persistence";

/// Cloneable durable setup coordinator composed with one runtime provider.
#[derive(Clone)]
pub struct SetupService {
    provider: Arc<dyn RuntimeProvider>,
    runtime: RuntimeService,
    repository: SetupJobRepository,
    capability: CapabilityEngine,
    active: Arc<Mutex<HashMap<SetupJobId, ActiveAttempt>>>,
    coordination: Arc<tokio::sync::Mutex<()>>,
    #[cfg(test)]
    fail_next_progress_persistence: Arc<std::sync::atomic::AtomicBool>,
    #[cfg(test)]
    pause_after_admission: Arc<std::sync::atomic::AtomicBool>,
    #[cfg(test)]
    admission_reached: Arc<tokio::sync::Notify>,
    #[cfg(test)]
    admission_release: Arc<tokio::sync::Notify>,
}

#[derive(Clone)]
struct ActiveAttempt {
    attempt_id: RequestId,
    cancellation: RuntimeCancellationToken,
}

impl SetupService {
    /// Creates a setup service backed by Rust-owned SQLite persistence.
    #[must_use]
    pub fn with_persistence(provider: Arc<dyn RuntimeProvider>, persistence: &Persistence) -> Self {
        Self {
            runtime: RuntimeService::with_persistence(provider.clone(), persistence),
            provider,
            repository: persistence.setup_jobs(),
            capability: CapabilityEngine::v0_1(),
            active: Arc::new(Mutex::new(HashMap::new())),
            coordination: Arc::new(tokio::sync::Mutex::new(())),
            #[cfg(test)]
            fail_next_progress_persistence: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            #[cfg(test)]
            pause_after_admission: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            #[cfg(test)]
            admission_reached: Arc::new(tokio::sync::Notify::new()),
            #[cfg(test)]
            admission_release: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// Revalidates a recommendation, resolves its exact provider mapping, and persists review.
    pub async fn create_plan(
        &self,
        request: SetupPlanRequest,
    ) -> Result<SetupPlanResponse, CoreError> {
        self.capability
            .validate_setup_recommendation(&request.recommendation)?;
        if request.provider_id != *self.provider.provider_id()
            || request.destination != SetupDestinationCategory::ProviderManaged
        {
            return Err(CoreError::InvalidSetupPlan);
        }
        if self
            .load_latest()
            .await?
            .is_some_and(|job| !job.state.is_terminal())
        {
            return Err(CoreError::SetupStateConflict);
        }

        let runtime_context = runtime_context(
            request.correlation_id,
            request.request_id,
            RuntimeCancellationToken::new(),
            PREPARATION_TIMEOUT,
        );
        let health = self
            .runtime
            .status(&request.provider_id, runtime_context.clone())
            .await
            .map_err(map_runtime_for_request)?;
        ensure_runtime_ready(&health)?;
        let acquisition_plan = self
            .provider
            .prepare_model_acquisition(
                request.recommendation.model.catalogue_id.clone(),
                runtime_context,
            )
            .await
            .map_err(map_runtime_for_request)?;
        validate_prepared_plan(&request, &acquisition_plan)?;

        let now = unix_timestamp_millis();
        let job_id = SetupJobId::new();
        let external_runtime_effect = health.ownership == RuntimeOwnership::External;
        let persisted = PersistedSetupPlan {
            job_id: persistence_job_id(job_id)?,
            plan_schema_version: SETUP_WORKFLOW_SCHEMA_VERSION,
            plan_revision: 1,
            canonical_model_id: request.recommendation.model.catalogue_id.clone(),
            model_display_name: request.recommendation.model.display_name.clone(),
            model_family: request.recommendation.model.family.clone(),
            model_size_class: request.recommendation.model.size_class,
            licence_spdx: request.recommendation.model.licence_spdx.clone(),
            provenance_url: request.recommendation.model.provenance_url.clone(),
            provider_id: request.provider_id.clone(),
            provider_model_id: acquisition_plan.artifact.provider_model_id.clone(),
            artifact_source_summary: acquisition_plan.artifact.source_summary.clone(),
            destination_display: acquisition_plan.destination_display.clone(),
            runtime_display_name: health.display_name.as_str().to_owned(),
            runtime_version: health.version.as_ref().map(|version| {
                version
                    .normalized_version
                    .clone()
                    .unwrap_or_else(|| version.reported_version.clone())
            }),
            catalogue_version: request.recommendation.catalogue_version.clone(),
            rule_set_version: request.recommendation.rule_set_version.clone(),
            destination_category: persisted_destination(acquisition_plan.destination)?,
            expected_size_bytes: request.recommendation.resources.storage.required_bytes,
            expected_memory_bytes: request.recommendation.resources.memory.required_bytes,
            memory_safety_margin_bytes: request.recommendation.resources.memory.safety_margin_bytes,
            storage_safety_margin_bytes: request
                .recommendation
                .resources
                .storage
                .safety_margin_bytes,
            planned_context_tokens: request.recommendation.resources.planned_context_tokens,
            cpu_only: request.recommendation.resources.cpu_only,
            gpu_memory_bytes: request.recommendation.resources.gpu_memory_bytes,
            // The cross-process request cannot attest scan-time observations. Keep the
            // approval plan conservative instead of persisting caller-supplied evidence.
            observed_total_memory_bytes: None,
            observed_available_memory_bytes: None,
            observed_free_storage_bytes: None,
            acceleration: request.recommendation.resources.acceleration,
            external_runtime_effect,
            notices: plan_notices(acquisition_plan.cancellation, external_runtime_effect),
            effect_codes: planned_effect_codes(),
            created_at_unix_ms: now,
        };
        let repository = self.repository.clone();
        let correlation_id = request.correlation_id;
        let job = blocking(move || repository.create(&persisted, correlation_id)).await?;
        let snapshot = self.snapshot(job, request.request_id).await?;
        Ok(SetupPlanResponse {
            job: snapshot,
            correlation_id: request.correlation_id,
            request_id: request.request_id,
        })
    }

    /// Records the exact persisted-plan decision before any provider effect can start.
    pub async fn decide_approval(
        &self,
        request: SetupApprovalRequest,
    ) -> Result<SetupApprovalResponse, CoreError> {
        let _coordination = self.coordination.lock().await;
        let decision = match request.decision {
            SetupApprovalDecision::Approve => PersistedSetupApprovalDecision::Approve,
            SetupApprovalDecision::Deny => PersistedSetupApprovalDecision::Deny,
            _ => return Err(CoreError::InvalidSetupPlan),
        };
        let repository = self.repository.clone();
        let input = PersistedSetupApprovalInput {
            job_id: persistence_job_id(request.job_id)?,
            plan_revision: request.plan_revision,
            decision,
            correlation_id: request.correlation_id,
            request_id: request.request_id,
            decided_at_unix_ms: unix_timestamp_millis(),
        };
        blocking(move || repository.decide_approval(&input)).await?;
        let approval = self
            .load_approval(request.job_id)
            .await?
            .ok_or(CoreError::SetupStateConflict)?;
        let job = self.load_job(request.job_id).await?;
        Ok(SetupApprovalResponse {
            job: self.snapshot(job, request.request_id).await?,
            approval: contract_approval(approval)?,
            correlation_id: request.correlation_id,
            request_id: request.request_id,
        })
    }

    /// Starts one approved attempt and returns after its first durable active checkpoint.
    pub async fn start_job(
        &self,
        request: SetupJobStartRequest,
    ) -> Result<SetupJobStartResponse, CoreError> {
        let job = self
            .start_or_retry(
                request.job_id,
                request.plan_revision,
                request.correlation_id,
                request.request_id,
                false,
            )
            .await?;
        Ok(SetupJobStartResponse {
            job,
            correlation_id: request.correlation_id,
            request_id: request.request_id,
        })
    }

    /// Returns the authoritative durable setup snapshot.
    pub async fn status(
        &self,
        request: SetupJobStatusRequest,
    ) -> Result<SetupJobStatusResponse, CoreError> {
        let job = self.load_job(request.job_id).await?;
        Ok(SetupJobStatusResponse {
            job: self.snapshot(job, request.request_id).await?,
            correlation_id: request.correlation_id,
            request_id: request.request_id,
        })
    }

    /// Recovers the newest durable job and classifies interrupted work conservatively.
    pub async fn recover_job(
        &self,
        request: SetupJobRecoveryRequest,
    ) -> Result<SetupJobRecoveryResponse, CoreError> {
        let interrupted = {
            let repository = self.repository.clone();
            blocking(move || repository.interrupted_jobs()).await?
        };
        for job in interrupted {
            let job_id = contract_job_id(&job.plan.job_id)?;
            if !self.is_active(job_id)? {
                let retained_effect_possible = acquisition_may_have_started(job.state);
                let mut transition = basic_transition(
                    &job,
                    PersistedSetupState::AttentionRequired,
                    request.correlation_id,
                )?;
                transition.attention_reason = Some("recovery_required".to_owned());
                transition.safe_error_code = Some("setup.recovery_required".to_owned());
                transition.event_kind = PersistedSetupEventKind::Recovery;
                transition.artifact_update = Some(PersistedArtifactUpdate {
                    lifecycle: if retained_effect_possible {
                        PersistedModelLifecycle::RetainedUnverified
                    } else {
                        PersistedModelLifecycle::Planned
                    },
                    verification: if retained_effect_possible {
                        PersistedModelVerification::Unknown
                    } else {
                        PersistedModelVerification::NotStarted
                    },
                    integrity: if retained_effect_possible {
                        PersistedModelIntegrity::Unknown
                    } else {
                        PersistedModelIntegrity::Unavailable
                    },
                    measured_size_bytes: None,
                    registered_at_unix_ms: None,
                    verified_at_unix_ms: None,
                });
                if let Some(effect_code) = pending_effect_for_state(job.state) {
                    transition.effects.push(PersistedSetupEffectInput {
                        disposition: PersistedEffectDisposition::Uncertain,
                        effect_code: effect_code.to_owned(),
                        size_bytes: None,
                    });
                }
                self.persist_transition(transition).await?;
            }
        }
        let job = match self.load_latest().await? {
            Some(job) => Some(self.snapshot(job, request.request_id).await?),
            None => None,
        };
        Ok(SetupJobRecoveryResponse {
            job,
            correlation_id: request.correlation_id,
            request_id: request.request_id,
        })
    }

    /// Returns one bounded page after an exclusive durable event cursor.
    pub async fn events_after(
        &self,
        request: SetupJobEventsRequest,
    ) -> Result<SetupJobEventsResponse, CoreError> {
        if request.limit == 0 || request.limit > EVENT_PAGE_MAX {
            return Err(CoreError::InvalidSetupPlan);
        }
        let job_id = persistence_job_id(request.job_id)?;
        let repository = self.repository.clone();
        let limit = u16::try_from(request.limit).map_err(|_| CoreError::InvalidSetupPlan)?;
        let events =
            blocking(move || repository.events_after(&job_id, request.after_sequence, limit))
                .await?;
        let current = self.load_job(request.job_id).await?;
        let current_snapshot = self.snapshot(current.clone(), request.request_id).await?;
        let mut mapped = Vec::with_capacity(events.len());
        for event in events {
            mapped.push(contract_event(
                &event,
                &current_snapshot,
                request.request_id,
            )?);
        }
        let next_after_sequence = mapped
            .last()
            .map_or(request.after_sequence, |event| event.sequence);
        Ok(SetupJobEventsResponse {
            has_more: next_after_sequence < current.last_event_sequence,
            events: mapped,
            next_after_sequence,
            correlation_id: request.correlation_id,
            request_id: request.request_id,
        })
    }

    /// Persists cancellation intent before signalling provider work.
    pub async fn cancel_job(
        &self,
        request: SetupJobCancelRequest,
    ) -> Result<SetupJobCancelResponse, CoreError> {
        let _coordination = self.coordination.lock().await;
        let job = self.load_job(request.job_id).await?;
        if job.state.is_terminal() {
            return Ok(SetupJobCancelResponse {
                accepted: false,
                job: self.snapshot(job, request.request_id).await?,
                correlation_id: request.correlation_id,
                request_id: request.request_id,
            });
        }
        let repository = self.repository.clone();
        let job_id = persistence_job_id(request.job_id)?;
        let correlation_id = request.correlation_id;
        request_cancellation_with_contention(repository, job_id, correlation_id).await?;
        let token = self
            .active
            .lock()
            .map_err(|_| CoreError::SetupStateConflict)?
            .get(&request.job_id)
            .map(|attempt| attempt.cancellation.clone());
        if let Some(token) = token {
            token.cancel();
        }
        self.finalize_cancellation(request.job_id, request.correlation_id)
            .await?;
        let updated = self.load_job(request.job_id).await?;
        Ok(SetupJobCancelResponse {
            accepted: true,
            job: self.snapshot(updated, request.request_id).await?,
            correlation_id: request.correlation_id,
            request_id: request.request_id,
        })
    }

    /// Retries the same exact approved plan after clearing all stale verification evidence.
    pub async fn retry_job(
        &self,
        request: SetupJobRetryRequest,
    ) -> Result<SetupJobRetryResponse, CoreError> {
        let job = self
            .start_or_retry(
                request.job_id,
                request.plan_revision,
                request.correlation_id,
                request.request_id,
                true,
            )
            .await?;
        Ok(SetupJobRetryResponse {
            job,
            correlation_id: request.correlation_id,
            request_id: request.request_id,
        })
    }

    async fn start_or_retry(
        &self,
        job_id: SetupJobId,
        plan_revision: u32,
        correlation_id: CorrelationId,
        request_id: RequestId,
        retry: bool,
    ) -> Result<SetupJobSnapshot, CoreError> {
        let _coordination = self.coordination.lock().await;
        let job = self.load_job(job_id).await?;
        let approval = self
            .load_approval(job_id)
            .await?
            .ok_or(CoreError::SetupApprovalRequired)?;
        if approval.decision != PersistedSetupApprovalDecision::Approve
            || approval.plan_revision != plan_revision
            || job.plan.plan_revision != plan_revision
        {
            return Err(CoreError::SetupApprovalRequired);
        }
        if retry && job.retry_count >= MAX_SETUP_RETRIES {
            if job.state != PersistedSetupState::AttentionRequired {
                return Err(CoreError::InvalidSetupPlan);
            }
            let mut exhausted =
                basic_transition(&job, PersistedSetupState::Failed, correlation_id)?;
            exhausted.safe_error_code = Some("setup.retry_limit_reached".to_owned());
            exhausted.event_kind = PersistedSetupEventKind::Terminal;
            self.persist_transition(exhausted).await?;
            return self
                .snapshot(self.load_job(job_id).await?, request_id)
                .await;
        }
        self.capability
            .validate_setup_versions(&job.plan.catalogue_version, &job.plan.rule_set_version)?;
        if retry && non_retriable_failure(&job) {
            return Err(CoreError::InvalidSetupPlan);
        }
        if job.cancellation_requested && !retry {
            self.persist_transition(cancelled_transition(
                &job,
                correlation_id,
                pending_effect_for_state(job.state)
                    .map(|effect_code| (PersistedEffectDisposition::Uncertain, effect_code)),
            )?)
            .await?;
            return Err(CoreError::SetupStateConflict);
        }
        let token = RuntimeCancellationToken::new();
        let attempt_id = RequestId::new();
        let admitted = {
            let mut active = self
                .active
                .lock()
                .map_err(|_| CoreError::SetupStateConflict)?;
            match active.entry(job_id) {
                Entry::Occupied(_) => false,
                Entry::Vacant(entry) => {
                    entry.insert(ActiveAttempt {
                        attempt_id,
                        cancellation: token.clone(),
                    });
                    true
                }
            }
        };
        if !admitted {
            return self
                .snapshot(self.load_job(job_id).await?, request_id)
                .await;
        }
        #[cfg(test)]
        if self
            .pause_after_admission
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            self.admission_reached.notify_waiters();
            self.admission_release.notified().await;
        }
        let valid_state = if retry {
            matches!(
                job.state,
                PersistedSetupState::AttentionRequired
                    | PersistedSetupState::Failed
                    | PersistedSetupState::Cancelled
            )
        } else {
            job.state == PersistedSetupState::Approved
        };
        if !valid_state {
            self.remove_active(job_id, attempt_id);
            return Err(CoreError::SetupStateConflict);
        }
        let transition = attempt_start_transition(&job, correlation_id, retry)?;
        if let Err(error) = self.persist_transition(transition).await {
            self.remove_active(job_id, attempt_id);
            return Err(error);
        }
        let snapshot = match self
            .snapshot(self.load_job(job_id).await?, request_id)
            .await
        {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.remove_active(job_id, attempt_id);
                return Err(error);
            }
        };
        let worker = self.clone();
        tokio::spawn(async move {
            if let Err(error) = worker
                .run_attempt(job_id, correlation_id, request_id, token)
                .await
            {
                tracing::warn!(
                    correlation_id = %correlation_id,
                    request_id = %request_id,
                    error = %error,
                    "model setup attempt stopped"
                );
            }
            worker.remove_active(job_id, attempt_id);
        });
        Ok(snapshot)
    }

    async fn persist_transition(
        &self,
        transition: PersistedSetupTransition,
    ) -> Result<(), CoreError> {
        validate_transition(
            transition.expected_state,
            transition.next_state,
            transition.next_stage,
        )?;
        let repository = self.repository.clone();
        blocking(move || repository.transition(&transition))
            .await
            .map(|_| ())
    }

    async fn finalize_cancellation(
        &self,
        job_id: SetupJobId,
        correlation_id: CorrelationId,
    ) -> Result<(), CoreError> {
        for attempt in 0..=PERSISTENCE_CONTENTION_RETRIES {
            let current = self.load_job(job_id).await?;
            if current.state.is_terminal() {
                return Ok(());
            }
            let transition = cancelled_transition(
                &current,
                correlation_id,
                pending_effect_for_state(current.state)
                    .map(|effect_code| (PersistedEffectDisposition::Uncertain, effect_code)),
            )?;
            match self.persist_transition(transition).await {
                Ok(()) => return Ok(()),
                Err(CoreError::SetupStateConflict) if attempt < PERSISTENCE_CONTENTION_RETRIES => {
                    tokio::task::yield_now().await;
                }
                Err(error) => return Err(error),
            }
        }
        Err(CoreError::SetupStateConflict)
    }

    async fn load_job(&self, job_id: SetupJobId) -> Result<PersistedSetupJob, CoreError> {
        let repository = self.repository.clone();
        let job_id = persistence_job_id(job_id)?;
        blocking(move || repository.get(&job_id))
            .await?
            .ok_or(CoreError::SetupJobNotFound)
    }

    async fn load_latest(&self) -> Result<Option<PersistedSetupJob>, CoreError> {
        let repository = self.repository.clone();
        blocking(move || repository.latest_recoverable()).await
    }

    async fn load_approval(
        &self,
        job_id: SetupJobId,
    ) -> Result<Option<PersistedSetupApproval>, CoreError> {
        let repository = self.repository.clone();
        let job_id = persistence_job_id(job_id)?;
        blocking(move || repository.approval(&job_id)).await
    }

    fn is_active(&self, job_id: SetupJobId) -> Result<bool, CoreError> {
        self.active
            .lock()
            .map_err(|_| CoreError::SetupStateConflict)
            .map(|active| active.contains_key(&job_id))
    }

    fn remove_active(&self, job_id: SetupJobId, attempt_id: RequestId) {
        if let Ok(mut active) = self.active.lock()
            && active
                .get(&job_id)
                .is_some_and(|attempt| attempt.attempt_id == attempt_id)
        {
            active.remove(&job_id);
        }
    }

    async fn run_attempt(
        &self,
        job_id: SetupJobId,
        correlation_id: CorrelationId,
        request_id: RequestId,
        cancellation: RuntimeCancellationToken,
    ) -> Result<(), CoreError> {
        let job = self.load_job(job_id).await?;
        let context = runtime_context(
            correlation_id,
            request_id,
            cancellation.clone(),
            PREPARATION_TIMEOUT,
        );
        let prepared = match self
            .provider
            .prepare_model_acquisition(job.plan.canonical_model_id.clone(), context)
            .await
        {
            Ok(plan) => plan,
            Err(error) => {
                return self
                    .record_runtime_outcome(job_id, correlation_id, error, None)
                    .await;
            }
        };
        if !prepared_matches_job(&prepared, &job) {
            return self
                .record_attention(
                    job_id,
                    correlation_id,
                    "provider_mapping_changed",
                    "setup.provider_mapping_changed",
                    false,
                )
                .await;
        }

        let runtime_health = self
            .runtime
            .status(
                &job.plan.provider_id,
                runtime_context(
                    correlation_id,
                    request_id,
                    cancellation.clone(),
                    PREPARATION_TIMEOUT,
                ),
            )
            .await;
        match runtime_health {
            Ok(health)
                if ensure_runtime_ready(&health).is_ok() && runtime_matches_plan(&health, &job) => {
            }
            Ok(_) => {
                return self
                    .record_attention(
                        job_id,
                        correlation_id,
                        "runtime_unavailable",
                        "setup.runtime_unavailable",
                        false,
                    )
                    .await;
            }
            Err(error) => {
                return self
                    .record_runtime_outcome(job_id, correlation_id, error, None)
                    .await;
            }
        }

        let current = self.load_job(job_id).await?;
        self.persist_transition(basic_transition(
            &current,
            PersistedSetupState::CheckingStorage,
            correlation_id,
        )?)
        .await?;
        let reusable = match self
            .provider
            .inspect_model(
                prepared.artifact.clone(),
                runtime_context(
                    correlation_id,
                    request_id,
                    cancellation.clone(),
                    MODEL_INSPECTION_TIMEOUT,
                ),
            )
            .await
        {
            Ok(inspection) => {
                if inspection.integrity == ModelIntegrityState::Mismatch {
                    return self
                        .record_integrity_mismatch(
                            job_id,
                            correlation_id,
                            inspection.registration.measured_size_bytes,
                        )
                        .await;
                }
                reusable_inspection(&prepared, &inspection).then_some(inspection)
            }
            Err(RuntimeError::ModelUnavailable | RuntimeError::ModelRegistrationFailed) => None,
            Err(error) => {
                return self
                    .record_runtime_outcome(job_id, correlation_id, error, None)
                    .await;
            }
        };
        let initial_required_bytes = if reusable.is_some() {
            0
        } else {
            job.plan.expected_size_bytes
        };
        if let Err(availability) = self
            .check_storage(
                &prepared,
                &job,
                correlation_id,
                request_id,
                cancellation.clone(),
                initial_required_bytes,
            )
            .await
        {
            return self
                .record_storage_attention(job_id, correlation_id, availability, false)
                .await;
        }

        let current = self.load_job(job_id).await?;
        let mut acquiring =
            basic_transition(&current, PersistedSetupState::Acquiring, correlation_id)?;
        acquiring.artifact_update = Some(PersistedArtifactUpdate {
            lifecycle: PersistedModelLifecycle::Acquiring,
            verification: PersistedModelVerification::Pending,
            integrity: PersistedModelIntegrity::Unavailable,
            measured_size_bytes: None,
            registered_at_unix_ms: None,
            verified_at_unix_ms: None,
        });
        self.persist_transition(acquiring).await?;

        let acquisition = if let Some(inspection) = reusable {
            RuntimeModelAcquisitionResult {
                artifact: inspection.artifact,
                status: RuntimeModelAcquisitionStatus::AlreadyPresent,
                measured_size_bytes: inspection.registration.measured_size_bytes,
                integrity: inspection.integrity,
                completed_at_unix_ms: inspection.registration.verified_at_unix_ms,
            }
        } else {
            match self
                .acquire_with_progress(
                    job_id,
                    &job,
                    prepared.clone(),
                    correlation_id,
                    request_id,
                    cancellation.clone(),
                )
                .await
            {
                Ok(result) => result,
                Err(AttemptError::Runtime(error)) => {
                    return self
                        .record_runtime_outcome(
                            job_id,
                            correlation_id,
                            error,
                            Some(prepared.cancellation),
                        )
                        .await;
                }
                Err(AttemptError::Storage(availability)) => {
                    cancellation.cancel();
                    return self
                        .record_storage_attention(job_id, correlation_id, availability, true)
                        .await;
                }
                Err(AttemptError::Core(error)) => {
                    if cancellation.is_cancelled()
                        || self.load_job(job_id).await?.cancellation_requested
                    {
                        return self
                            .record_runtime_outcome(
                                job_id,
                                correlation_id,
                                RuntimeError::Cancelled,
                                Some(prepared.cancellation),
                            )
                            .await;
                    }
                    return match self
                        .record_attention(
                            job_id,
                            correlation_id,
                            "acquisition_interrupted",
                            "setup.progress_persistence_failed",
                            true,
                        )
                        .await
                    {
                        Ok(()) => Ok(()),
                        Err(_) => Err(error),
                    };
                }
            }
        };
        if cancellation.is_cancelled() || self.load_job(job_id).await?.cancellation_requested {
            let current = self.load_job(job_id).await?;
            return self
                .persist_transition(cancelled_transition(
                    &current,
                    correlation_id,
                    pending_effect_for_state(current.state)
                        .map(|effect_code| (PersistedEffectDisposition::Uncertain, effect_code)),
                )?)
                .await;
        }
        if acquisition.artifact.canonical_model_id != job.plan.canonical_model_id
            || acquisition.artifact.provider_id != job.plan.provider_id
            || acquisition.artifact.provider_model_id != job.plan.provider_model_id
        {
            return self
                .record_attention(
                    job_id,
                    correlation_id,
                    "provider_mapping_changed",
                    "setup.provider_mapping_changed",
                    true,
                )
                .await;
        }
        if acquisition.integrity == ModelIntegrityState::Mismatch {
            return self
                .record_integrity_mismatch(job_id, correlation_id, acquisition.measured_size_bytes)
                .await;
        }

        let current = self.load_job(job_id).await?;
        let mut registering =
            basic_transition(&current, PersistedSetupState::Registering, correlation_id)?;
        registering.artifact_update = Some(PersistedArtifactUpdate {
            lifecycle: PersistedModelLifecycle::AcquiredUnverified,
            verification: PersistedModelVerification::Pending,
            integrity: persisted_integrity(acquisition.integrity),
            measured_size_bytes: acquisition.measured_size_bytes,
            registered_at_unix_ms: None,
            verified_at_unix_ms: None,
        });
        if acquisition.status == RuntimeModelAcquisitionStatus::Acquired {
            registering.effects.push(PersistedSetupEffectInput {
                disposition: PersistedEffectDisposition::Completed,
                effect_code: EFFECT_ACQUISITION.to_owned(),
                size_bytes: acquisition.measured_size_bytes,
            });
        }
        self.persist_transition(registering).await?;

        if let Err(availability) = self
            .check_storage(
                &prepared,
                &job,
                correlation_id,
                request_id,
                cancellation.clone(),
                0,
            )
            .await
        {
            return self
                .record_storage_attention(job_id, correlation_id, availability, true)
                .await;
        }
        let current = self.load_job(job_id).await?;
        self.persist_transition(basic_transition(
            &current,
            PersistedSetupState::VerifyingRuntime,
            correlation_id,
        )?)
        .await?;

        let health = match self
            .runtime
            .status(
                &job.plan.provider_id,
                runtime_context(
                    correlation_id,
                    request_id,
                    cancellation.clone(),
                    PREPARATION_TIMEOUT,
                ),
            )
            .await
        {
            Ok(health)
                if ensure_runtime_ready(&health).is_ok() && runtime_matches_plan(&health, &job) =>
            {
                health
            }
            Ok(_) => {
                return self
                    .record_attention(
                        job_id,
                        correlation_id,
                        "runtime_unavailable",
                        "setup.runtime_unavailable",
                        true,
                    )
                    .await;
            }
            Err(error) => {
                return self
                    .record_runtime_outcome(
                        job_id,
                        correlation_id,
                        error,
                        Some(prepared.cancellation),
                    )
                    .await;
            }
        };
        let _ = health;
        let runtime_verified_at = unix_timestamp_millis();
        let current = self.load_job(job_id).await?;
        let mut verifying_model = basic_transition(
            &current,
            PersistedSetupState::VerifyingModel,
            correlation_id,
        )?;
        verifying_model.runtime_verified_at_unix_ms = Some(runtime_verified_at);
        verifying_model.artifact_update = Some(PersistedArtifactUpdate {
            lifecycle: PersistedModelLifecycle::Verifying,
            verification: PersistedModelVerification::Pending,
            integrity: persisted_integrity(acquisition.integrity),
            measured_size_bytes: acquisition.measured_size_bytes,
            registered_at_unix_ms: None,
            verified_at_unix_ms: None,
        });
        self.persist_transition(verifying_model).await?;

        let inspection = match self
            .provider
            .inspect_model(
                prepared.artifact.clone(),
                runtime_context(
                    correlation_id,
                    request_id,
                    cancellation.clone(),
                    MODEL_INSPECTION_TIMEOUT,
                ),
            )
            .await
        {
            Ok(inspection) => inspection,
            Err(error) => {
                return self
                    .record_runtime_outcome(
                        job_id,
                        correlation_id,
                        error,
                        Some(prepared.cancellation),
                    )
                    .await;
            }
        };
        if inspection.artifact != prepared.artifact
            || !inspection.available
            || inspection.registration.state != ProviderRegistrationState::Registered
        {
            return self
                .record_attention(
                    job_id,
                    correlation_id,
                    "registration_unverified",
                    "setup.registration_unverified",
                    true,
                )
                .await;
        }
        if inspection.integrity == ModelIntegrityState::Mismatch {
            return self
                .record_integrity_mismatch(
                    job_id,
                    correlation_id,
                    inspection
                        .registration
                        .measured_size_bytes
                        .or(acquisition.measured_size_bytes),
                )
                .await;
        }
        if !matches!(
            inspection.integrity,
            ModelIntegrityState::ProviderReported | ModelIntegrityState::Verified
        ) {
            return self
                .record_attention(
                    job_id,
                    correlation_id,
                    "acquisition_interrupted",
                    "setup.integrity_unverified",
                    true,
                )
                .await;
        }
        let verified_at = i64::try_from(inspection.registration.verified_at_unix_ms)
            .map_err(|_| CoreError::InvalidSetupPlan)?;
        let current = self.load_job(job_id).await?;
        let mut inference = basic_transition(
            &current,
            PersistedSetupState::RunningTestInference,
            correlation_id,
        )?;
        inference.model_available_at_unix_ms = Some(verified_at);
        inference.registration_verified_at_unix_ms = Some(verified_at);
        inference.artifact_update = Some(PersistedArtifactUpdate {
            lifecycle: PersistedModelLifecycle::Verifying,
            verification: PersistedModelVerification::Pending,
            integrity: persisted_integrity(inspection.integrity),
            measured_size_bytes: inspection
                .registration
                .measured_size_bytes
                .or(acquisition.measured_size_bytes),
            registered_at_unix_ms: Some(verified_at),
            verified_at_unix_ms: None,
        });
        inference.effects.push(PersistedSetupEffectInput {
            disposition: PersistedEffectDisposition::Completed,
            effect_code: EFFECT_REGISTRATION.to_owned(),
            size_bytes: inspection.registration.measured_size_bytes,
        });
        self.persist_transition(inference).await?;

        if let Err(availability) = self
            .check_storage(
                &prepared,
                &job,
                correlation_id,
                request_id,
                cancellation.clone(),
                0,
            )
            .await
        {
            return self
                .record_storage_attention(job_id, correlation_id, availability, true)
                .await;
        }
        let readiness = match self
            .provider
            .run_readiness_inference(
                prepared.artifact.clone(),
                runtime_context(
                    correlation_id,
                    request_id,
                    cancellation.clone(),
                    READINESS_TIMEOUT,
                ),
            )
            .await
        {
            Ok(readiness) if readiness.ready => readiness,
            Ok(_) => {
                return self
                    .record_attention(
                        job_id,
                        correlation_id,
                        "readiness_failed",
                        "setup.readiness_failed",
                        true,
                    )
                    .await;
            }
            Err(error) => {
                return self
                    .record_runtime_outcome(
                        job_id,
                        correlation_id,
                        error,
                        Some(prepared.cancellation),
                    )
                    .await;
            }
        };
        let inference_at = i64::try_from(readiness.completed_at_unix_ms)
            .map_err(|_| CoreError::InvalidSetupPlan)?;
        let current = self.load_job(job_id).await?;
        if cancellation.is_cancelled() || current.cancellation_requested {
            return self
                .persist_transition(cancelled_transition(
                    &current,
                    correlation_id,
                    pending_effect_for_state(current.state)
                        .map(|effect_code| (PersistedEffectDisposition::Uncertain, effect_code)),
                )?)
                .await;
        }
        let mut ready = basic_transition(&current, PersistedSetupState::Ready, correlation_id)?;
        ready.runtime_verified_at_unix_ms = Some(runtime_verified_at);
        ready.model_available_at_unix_ms = Some(verified_at);
        ready.registration_verified_at_unix_ms = Some(verified_at);
        ready.inference_verified_at_unix_ms = Some(inference_at);
        ready.event_kind = PersistedSetupEventKind::Terminal;
        ready.artifact_update = Some(PersistedArtifactUpdate {
            lifecycle: PersistedModelLifecycle::Available,
            verification: PersistedModelVerification::Verified,
            integrity: persisted_integrity(inspection.integrity),
            measured_size_bytes: inspection
                .registration
                .measured_size_bytes
                .or(acquisition.measured_size_bytes),
            registered_at_unix_ms: Some(verified_at),
            verified_at_unix_ms: Some(inference_at),
        });
        ready.effects.extend([
            PersistedSetupEffectInput {
                disposition: PersistedEffectDisposition::Completed,
                effect_code: EFFECT_INFERENCE.to_owned(),
                size_bytes: None,
            },
            PersistedSetupEffectInput {
                disposition: PersistedEffectDisposition::Completed,
                effect_code: EFFECT_METADATA.to_owned(),
                size_bytes: None,
            },
        ]);
        self.persist_transition(ready).await
    }

    async fn acquire_with_progress(
        &self,
        job_id: SetupJobId,
        job: &PersistedSetupJob,
        plan: RuntimeModelAcquisitionPlan,
        correlation_id: CorrelationId,
        request_id: RequestId,
        cancellation: RuntimeCancellationToken,
    ) -> Result<gixgiz_runtime::RuntimeModelAcquisitionResult, AttemptError> {
        let (sender, mut receiver) = tokio::sync::mpsc::channel(MODEL_PROGRESS_CHANNEL_CAPACITY);
        let provider = self.provider.clone();
        let acquisition_plan = plan.clone();
        let acquisition_context = runtime_context(
            correlation_id,
            request_id,
            cancellation.clone(),
            ACQUISITION_TIMEOUT,
        );
        let mut task = tokio::spawn(async move {
            provider
                .acquire_model(acquisition_plan, sender, acquisition_context)
                .await
        });
        let mut last_checkpoint_bytes = 0;
        let mut last_checkpoint_basis = 0;
        let mut last_phase = None;
        let mut last_observed_completed_bytes = 0;
        let mut progress_open = true;
        let mut storage_interval = tokio::time::interval_at(
            tokio::time::Instant::now() + STORAGE_CHECKPOINT_INTERVAL,
            STORAGE_CHECKPOINT_INTERVAL,
        );
        loop {
            tokio::select! {
                biased;
                progress = receiver.recv(), if progress_open => {
                    let Some(progress) = progress else {
                        progress_open = false;
                        continue;
                    };
                    if progress.phase == ModelAcquisitionPhase::Unknown {
                        continue;
                    }
                    let progress = match authoritative_progress(job.plan.expected_size_bytes, progress) {
                        Ok(progress) => progress,
                        Err(error) => {
                            abort_acquisition_task(&cancellation, &mut task).await;
                            return Err(AttemptError::Runtime(error));
                        }
                    };
                    if let Err(error) = self
                        .persist_progress(job_id, correlation_id, progress.clone())
                        .await
                    {
                        abort_acquisition_task(&cancellation, &mut task).await;
                        return Err(AttemptError::Core(error));
                    }
                    last_observed_completed_bytes = progress
                        .completed_bytes
                        .unwrap_or(last_observed_completed_bytes)
                        .min(job.plan.expected_size_bytes);
                    if storage_checkpoint_due(
                        &progress,
                        last_checkpoint_bytes,
                        last_checkpoint_basis,
                        last_phase,
                    ) {
                        match self.check_storage(
                            &plan,
                            job,
                            correlation_id,
                            request_id,
                            cancellation.clone(),
                            remaining_acquisition_bytes(
                                job.plan.expected_size_bytes,
                                last_observed_completed_bytes,
                            ),
                        ).await {
                            Ok(()) => {
                                last_checkpoint_bytes = progress.completed_bytes.unwrap_or(last_checkpoint_bytes);
                                last_checkpoint_basis = progress.progress_basis_points.unwrap_or(last_checkpoint_basis);
                                last_phase = Some(progress.phase);
                            }
                            Err(availability) => {
                                cancellation.cancel();
                                task.abort();
                                let _ = task.await;
                                return Err(AttemptError::Storage(availability));
                            }
                        }
                    }
                }
                _ = storage_interval.tick() => {
                    if let Err(availability) = self.check_storage(
                        &plan,
                        job,
                        correlation_id,
                        request_id,
                        cancellation.clone(),
                        remaining_acquisition_bytes(
                            job.plan.expected_size_bytes,
                            last_observed_completed_bytes,
                        ),
                    ).await {
                        cancellation.cancel();
                        task.abort();
                        let _ = task.await;
                        return Err(AttemptError::Storage(availability));
                    }
                }
                () = cancellation.cancelled() => {
                    abort_acquisition_task(&cancellation, &mut task).await;
                    return Err(AttemptError::Runtime(RuntimeError::Cancelled));
                }
                result = &mut task => {
                    let terminal = result
                        .map_err(|_| AttemptError::Runtime(RuntimeError::Internal))?
                        .map_err(AttemptError::Runtime)?;
                    while let Ok(progress) = receiver.try_recv() {
                        if progress.phase == ModelAcquisitionPhase::Unknown {
                            continue;
                        }
                        let progress = authoritative_progress(job.plan.expected_size_bytes, progress)
                            .map_err(AttemptError::Runtime)?;
                        self.persist_progress(job_id, correlation_id, progress.clone()).await
                            .map_err(AttemptError::Core)?;
                        last_observed_completed_bytes = progress
                            .completed_bytes
                            .unwrap_or(last_observed_completed_bytes)
                            .min(job.plan.expected_size_bytes);
                        if storage_checkpoint_due(
                            &progress,
                            last_checkpoint_bytes,
                            last_checkpoint_basis,
                            last_phase,
                        ) {
                            self.check_storage(
                                &plan,
                                job,
                                correlation_id,
                                request_id,
                                cancellation.clone(),
                                remaining_acquisition_bytes(
                                    job.plan.expected_size_bytes,
                                    last_observed_completed_bytes,
                                ),
                            ).await.map_err(AttemptError::Storage)?;
                            last_checkpoint_bytes = progress.completed_bytes.unwrap_or(last_checkpoint_bytes);
                            last_checkpoint_basis = progress.progress_basis_points.unwrap_or(last_checkpoint_basis);
                            last_phase = Some(progress.phase);
                        }
                    }
                    return Ok(terminal);
                }
            }
        }
    }

    async fn persist_progress(
        &self,
        job_id: SetupJobId,
        correlation_id: CorrelationId,
        progress: ModelAcquisitionProgress,
    ) -> Result<(), CoreError> {
        #[cfg(test)]
        if self
            .fail_next_progress_persistence
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Err(CoreError::SetupPersistenceUnavailable);
        }
        if progress.phase == ModelAcquisitionPhase::Unknown {
            return Ok(());
        }
        let job = self.load_job(job_id).await?;
        if job.state != PersistedSetupState::Acquiring {
            return Ok(());
        }
        let mut transition =
            basic_transition(&job, PersistedSetupState::Acquiring, correlation_id)?;
        let progress = authoritative_progress(job.plan.expected_size_bytes, progress)
            .map_err(|_| CoreError::InvalidSetupPlan)?;
        transition.progress = Some(PersistedSetupProgress {
            phase: Some(progress.phase),
            completed_bytes: progress.completed_bytes,
            total_bytes: progress.total_bytes,
            basis_points: progress.progress_basis_points,
        });
        transition.event_kind = PersistedSetupEventKind::Progress;
        self.persist_transition(transition).await
    }

    async fn check_storage(
        &self,
        plan: &RuntimeModelAcquisitionPlan,
        job: &PersistedSetupJob,
        correlation_id: CorrelationId,
        request_id: RequestId,
        cancellation: RuntimeCancellationToken,
        remaining_required_bytes: u64,
    ) -> Result<(), RuntimeStorageAvailability> {
        let result = self
            .provider
            .preflight_model_storage(
                plan.clone(),
                remaining_required_bytes,
                job.plan.storage_safety_margin_bytes,
                runtime_context(
                    correlation_id,
                    request_id,
                    cancellation,
                    PREPARATION_TIMEOUT,
                ),
            )
            .await
            .map_err(|error| match error {
                RuntimeError::ModelStorageExhausted => {
                    RuntimeStorageAvailability::InsufficientSpace
                }
                RuntimeError::ModelStorageUnavailable => RuntimeStorageAvailability::Unavailable,
                _ => RuntimeStorageAvailability::Unverified,
            })?;
        if result.destination != plan.destination
            || result.destination_display != plan.destination_display
            || result.required_bytes != remaining_required_bytes
            || result.safety_margin_bytes != job.plan.storage_safety_margin_bytes
        {
            return Err(RuntimeStorageAvailability::Unverified);
        }
        match result.availability {
            RuntimeStorageAvailability::Available => Ok(()),
            availability => Err(availability),
        }
    }

    async fn record_runtime_outcome(
        &self,
        job_id: SetupJobId,
        correlation_id: CorrelationId,
        error: RuntimeError,
        cancellation_semantics: Option<RuntimeCancellationSemantics>,
    ) -> Result<(), CoreError> {
        let job = self.load_job(job_id).await?;
        if job.state.is_terminal() {
            return Ok(());
        }
        if error == RuntimeError::Cancelled && job.cancellation_requested {
            let effect = match job.state {
                PersistedSetupState::Acquiring => Some((
                    match cancellation_semantics {
                        Some(RuntimeCancellationSemantics::Cooperative) => {
                            PersistedEffectDisposition::RolledBack
                        }
                        _ => PersistedEffectDisposition::Uncertain,
                    },
                    EFFECT_ACQUISITION,
                )),
                state => pending_effect_for_state(state)
                    .map(|effect_code| (PersistedEffectDisposition::Uncertain, effect_code)),
            };
            return self
                .persist_transition(cancelled_transition(&job, correlation_id, effect)?)
                .await;
        }
        if error == RuntimeError::ModelIntegrityFailed {
            return self
                .record_integrity_mismatch(job_id, correlation_id, None)
                .await;
        }
        if matches!(
            error,
            RuntimeError::InvalidInput | RuntimeError::Unsupported
        ) {
            return self
                .record_failed(
                    job_id,
                    correlation_id,
                    "setup.provider_contract_unsupported",
                    acquisition_may_have_started(job.state),
                )
                .await;
        }
        let (reason, code) = runtime_attention(&error, job.state);
        self.record_attention(
            job_id,
            correlation_id,
            reason,
            code,
            cancellation_semantics.is_some(),
        )
        .await
    }

    async fn record_storage_attention(
        &self,
        job_id: SetupJobId,
        correlation_id: CorrelationId,
        availability: RuntimeStorageAvailability,
        retained_effect_possible: bool,
    ) -> Result<(), CoreError> {
        let (reason, code) = match availability {
            RuntimeStorageAvailability::InsufficientSpace => {
                ("insufficient_storage", "setup.insufficient_storage")
            }
            RuntimeStorageAvailability::Unavailable => {
                ("destination_unavailable", "setup.destination_unavailable")
            }
            _ => ("destination_unavailable", "setup.storage_unverified"),
        };
        self.record_attention(
            job_id,
            correlation_id,
            reason,
            code,
            retained_effect_possible,
        )
        .await
    }

    async fn record_attention(
        &self,
        job_id: SetupJobId,
        correlation_id: CorrelationId,
        reason: &str,
        code: &str,
        retained_effect_possible: bool,
    ) -> Result<(), CoreError> {
        let job = self.load_job(job_id).await?;
        if job.state.is_terminal() {
            return Ok(());
        }
        let mut transition =
            basic_transition(&job, PersistedSetupState::AttentionRequired, correlation_id)?;
        transition.attention_reason = Some(reason.to_owned());
        transition.safe_error_code = Some(code.to_owned());
        transition.event_kind = PersistedSetupEventKind::Terminal;
        transition.artifact_update = Some(PersistedArtifactUpdate {
            lifecycle: if retained_effect_possible {
                PersistedModelLifecycle::RetainedUnverified
            } else {
                PersistedModelLifecycle::Planned
            },
            verification: PersistedModelVerification::Unknown,
            integrity: PersistedModelIntegrity::Unknown,
            measured_size_bytes: None,
            registered_at_unix_ms: None,
            verified_at_unix_ms: None,
        });
        if retained_effect_possible && let Some(effect_code) = pending_effect_for_state(job.state) {
            transition.effects.push(PersistedSetupEffectInput {
                disposition: PersistedEffectDisposition::Uncertain,
                effect_code: effect_code.to_owned(),
                size_bytes: None,
            });
        }
        self.persist_transition(transition).await
    }

    async fn record_failed(
        &self,
        job_id: SetupJobId,
        correlation_id: CorrelationId,
        code: &str,
        retained_effect_possible: bool,
    ) -> Result<(), CoreError> {
        let job = self.load_job(job_id).await?;
        if job.state.is_terminal() {
            return Ok(());
        }
        let mut transition = basic_transition(&job, PersistedSetupState::Failed, correlation_id)?;
        transition.safe_error_code = Some(code.to_owned());
        transition.event_kind = PersistedSetupEventKind::Terminal;
        transition.artifact_update = Some(PersistedArtifactUpdate {
            lifecycle: PersistedModelLifecycle::Failed,
            verification: PersistedModelVerification::Failed,
            integrity: PersistedModelIntegrity::Unknown,
            measured_size_bytes: None,
            registered_at_unix_ms: None,
            verified_at_unix_ms: None,
        });
        if retained_effect_possible && let Some(effect_code) = pending_effect_for_state(job.state) {
            transition.effects.push(PersistedSetupEffectInput {
                disposition: PersistedEffectDisposition::Uncertain,
                effect_code: effect_code.to_owned(),
                size_bytes: None,
            });
        }
        self.persist_transition(transition).await
    }

    async fn record_integrity_mismatch(
        &self,
        job_id: SetupJobId,
        correlation_id: CorrelationId,
        measured_size_bytes: Option<u64>,
    ) -> Result<(), CoreError> {
        let job = self.load_job(job_id).await?;
        if job.state.is_terminal() {
            return Ok(());
        }
        let mut transition = basic_transition(&job, PersistedSetupState::Failed, correlation_id)?;
        transition.safe_error_code = Some("setup.integrity_mismatch".to_owned());
        transition.event_kind = PersistedSetupEventKind::Terminal;
        transition.artifact_update = Some(PersistedArtifactUpdate {
            lifecycle: PersistedModelLifecycle::Failed,
            verification: PersistedModelVerification::Failed,
            integrity: PersistedModelIntegrity::Mismatch,
            measured_size_bytes,
            registered_at_unix_ms: None,
            verified_at_unix_ms: None,
        });
        if job.state == PersistedSetupState::Acquiring {
            transition.effects.push(PersistedSetupEffectInput {
                disposition: PersistedEffectDisposition::Retained,
                effect_code: EFFECT_ACQUISITION.to_owned(),
                size_bytes: measured_size_bytes,
            });
        }
        self.persist_transition(transition).await
    }

    async fn snapshot(
        &self,
        job: PersistedSetupJob,
        request_id: RequestId,
    ) -> Result<SetupJobSnapshot, CoreError> {
        let repository = self.repository.clone();
        let job_id = job.plan.job_id.clone();
        let canonical_model_id = job.plan.canonical_model_id.clone();
        let provider_id = job.plan.provider_id.clone();
        let (effects, artifact, events) = blocking(move || {
            Ok((
                repository.effects(&job_id)?,
                repository.artifact(&canonical_model_id, &provider_id)?,
                repository.events_after(&job_id, 0, EVENT_PAGE_MAX as u16)?,
            ))
        })
        .await?;
        build_snapshot(job, effects, artifact, events, request_id)
    }
}

enum AttemptError {
    Runtime(RuntimeError),
    Storage(RuntimeStorageAvailability),
    Core(CoreError),
}

async fn abort_acquisition_task(
    cancellation: &RuntimeCancellationToken,
    task: &mut tokio::task::JoinHandle<Result<RuntimeModelAcquisitionResult, RuntimeError>>,
) {
    cancellation.cancel();
    task.abort();
    let _ = task.await;
}

fn runtime_context(
    correlation_id: CorrelationId,
    request_id: RequestId,
    cancellation: RuntimeCancellationToken,
    timeout: Duration,
) -> RuntimeOperationContext {
    RuntimeOperationContext::new(correlation_id, request_id, timeout)
        .with_cancellation(cancellation)
}

fn ensure_runtime_ready(health: &gixgiz_contracts::RuntimeHealthReport) -> Result<(), CoreError> {
    if health.state != RuntimeState::Ready
        || (health.ownership == RuntimeOwnership::External
            && health.reuse_consent != RuntimeConsentState::ReuseApproved)
    {
        return Err(CoreError::SetupProviderUnavailable);
    }
    Ok(())
}

fn runtime_matches_plan(
    health: &gixgiz_contracts::RuntimeHealthReport,
    job: &PersistedSetupJob,
) -> bool {
    let external_runtime_effect = match health.ownership {
        RuntimeOwnership::External => true,
        RuntimeOwnership::GixGizManaged | RuntimeOwnership::Bundled => false,
        _ => return false,
    };
    let version = health.version.as_ref().map(|value| {
        value
            .normalized_version
            .clone()
            .unwrap_or_else(|| value.reported_version.clone())
    });
    health.provider_id == job.plan.provider_id
        && health.display_name.as_str() == job.plan.runtime_display_name
        && external_runtime_effect == job.plan.external_runtime_effect
        && version == job.plan.runtime_version
}

fn reusable_inspection(
    plan: &RuntimeModelAcquisitionPlan,
    inspection: &gixgiz_runtime::RuntimeModelInspection,
) -> bool {
    inspection.artifact == plan.artifact
        && inspection.available
        && inspection.registration.state == ProviderRegistrationState::Registered
        && matches!(
            inspection.integrity,
            ModelIntegrityState::ProviderReported | ModelIntegrityState::Verified
        )
}

const fn remaining_acquisition_bytes(expected_bytes: u64, completed_bytes: u64) -> u64 {
    expected_bytes.saturating_sub(completed_bytes)
}

fn authoritative_progress(
    expected_size_bytes: u64,
    mut observed: ModelAcquisitionProgress,
) -> Result<ModelAcquisitionProgress, RuntimeError> {
    if observed
        .completed_bytes
        .is_some_and(|value| value > expected_size_bytes)
        || observed
            .total_bytes
            .is_some_and(|value| value > expected_size_bytes)
    {
        return Err(RuntimeError::InvalidResponse);
    }
    let completed = observed.completed_bytes;
    observed.total_bytes = Some(expected_size_bytes);
    observed.progress_basis_points = completed.map(|value| {
        let basis_points = u128::from(value)
            .saturating_mul(10_000)
            .checked_div(u128::from(expected_size_bytes))
            .unwrap_or(0)
            .min(10_000);
        u16::try_from(basis_points).unwrap_or(10_000)
    });
    Ok(observed)
}

fn validate_prepared_plan(
    request: &SetupPlanRequest,
    plan: &RuntimeModelAcquisitionPlan,
) -> Result<(), CoreError> {
    if plan.artifact.canonical_model_id != request.recommendation.model.catalogue_id
        || plan.artifact.provider_id != request.provider_id
        || plan.destination != request.destination
        || !safe_provider_summary(&plan.artifact.source_summary)
        || !safe_provider_summary(&plan.destination_display)
    {
        return Err(CoreError::InvalidSetupPlan);
    }
    Ok(())
}

fn safe_provider_summary(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= SAFE_PROVIDER_SUMMARY_BYTES_MAX
        && !value
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\'))
}

fn map_runtime_for_request(_error: RuntimeError) -> CoreError {
    CoreError::SetupProviderUnavailable
}

async fn blocking<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, PersistenceError> + Send + 'static,
) -> Result<T, CoreError> {
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|_| CoreError::SetupPersistenceUnavailable)?
        .map_err(map_persistence)
}

async fn request_cancellation_with_contention(
    repository: SetupJobRepository,
    job_id: JobId,
    correlation_id: CorrelationId,
) -> Result<(), CoreError> {
    let occurred_at_unix_ms = unix_timestamp_millis();
    tokio::task::spawn_blocking(move || {
        for attempt in 0..=PERSISTENCE_CONTENTION_RETRIES {
            match repository.request_cancellation(&job_id, correlation_id, occurred_at_unix_ms) {
                Err(error)
                    if persistence_contention(&error)
                        && attempt < PERSISTENCE_CONTENTION_RETRIES =>
                {
                    std::thread::sleep(PERSISTENCE_CONTENTION_RETRY_DELAY);
                }
                result => return result,
            }
        }
        Err(PersistenceError::ConnectionUnavailable)
    })
    .await
    .map_err(|_| CoreError::SetupPersistenceUnavailable)?
    .map(|_| ())
    .map_err(map_persistence)
}

const fn persistence_contention(error: &PersistenceError) -> bool {
    matches!(
        error,
        PersistenceError::ConnectionUnavailable | PersistenceError::Locked { .. }
    )
}

fn map_persistence(error: PersistenceError) -> CoreError {
    match error {
        PersistenceError::RecordNotFound { .. } => CoreError::SetupJobNotFound,
        PersistenceError::RecordConflict { .. } => CoreError::SetupStateConflict,
        PersistenceError::InvalidRecord { .. } => CoreError::InvalidSetupPlan,
        _ => CoreError::SetupPersistenceUnavailable,
    }
}

fn unix_timestamp_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
        })
}

fn persistence_job_id(job_id: SetupJobId) -> Result<JobId, CoreError> {
    JobId::new(job_id.to_string()).map_err(map_persistence)
}

fn contract_job_id(job_id: &JobId) -> Result<SetupJobId, CoreError> {
    SetupJobId::from_str(job_id.as_str()).map_err(|_| CoreError::SetupPersistenceUnavailable)
}

fn persisted_destination(
    destination: SetupDestinationCategory,
) -> Result<PersistedDestinationCategory, CoreError> {
    match destination {
        SetupDestinationCategory::ProviderManaged => {
            Ok(PersistedDestinationCategory::ProviderManaged)
        }
        SetupDestinationCategory::ApplicationData => {
            Ok(PersistedDestinationCategory::ApplicationData)
        }
        SetupDestinationCategory::UserSelected => Ok(PersistedDestinationCategory::UserSelected),
        _ => Err(CoreError::InvalidSetupPlan),
    }
}

fn contract_destination(destination: PersistedDestinationCategory) -> SetupDestinationCategory {
    match destination {
        PersistedDestinationCategory::ProviderManaged => SetupDestinationCategory::ProviderManaged,
        PersistedDestinationCategory::ApplicationData => SetupDestinationCategory::ApplicationData,
        PersistedDestinationCategory::UserSelected => SetupDestinationCategory::UserSelected,
        _ => SetupDestinationCategory::Unknown,
    }
}

fn planned_effect_codes() -> Vec<String> {
    [
        EFFECT_ACQUISITION,
        EFFECT_REGISTRATION,
        EFFECT_INFERENCE,
        EFFECT_METADATA,
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn plan_notices(
    cancellation: RuntimeCancellationSemantics,
    external_runtime_effect: bool,
) -> Vec<PersistedSetupNotice> {
    let mut notices = vec![PersistedSetupNotice {
        warning: false,
        code: "recommendation_selected".to_owned(),
        message: "This model was selected by the deterministic local capability plan.".to_owned(),
    }];
    notices.push(PersistedSetupNotice {
        warning: true,
        code: "provider_managed_storage".to_owned(),
        message: "The runtime provider controls model storage and retained partial data."
            .to_owned(),
    });
    notices.push(PersistedSetupNotice {
        warning: true,
        code: "destination_evidence_incomplete".to_owned(),
        message: "The provider cannot attest its effective model-storage root.".to_owned(),
    });
    notices.push(PersistedSetupNotice {
        warning: true,
        code: "integrity_metadata_unavailable".to_owned(),
        message: "The catalogue has no trusted expected checksum for independent comparison."
            .to_owned(),
    });
    if external_runtime_effect {
        notices.push(PersistedSetupNotice {
            warning: true,
            code: "external_runtime_modified".to_owned(),
            message: "Approval adds a model to an externally managed local runtime.".to_owned(),
        });
    }
    if cancellation != RuntimeCancellationSemantics::Cooperative {
        notices.push(PersistedSetupNotice {
            warning: true,
            code: "cancellation_may_retain_effects".to_owned(),
            message: "Cancellation may retain provider-owned partial or completed model data."
                .to_owned(),
        });
    }
    notices.truncate(64);
    notices
}

fn contract_approval(approval: PersistedSetupApproval) -> Result<SetupApprovalRecord, CoreError> {
    Ok(SetupApprovalRecord {
        job_id: contract_job_id(&approval.job_id)?,
        plan_revision: approval.plan_revision,
        decision: match approval.decision {
            PersistedSetupApprovalDecision::Approve => SetupApprovalDecision::Approve,
            PersistedSetupApprovalDecision::Deny => SetupApprovalDecision::Deny,
            _ => SetupApprovalDecision::Unknown,
        },
        canonical_model_id: approval.canonical_model_id,
        provider_id: approval.provider_id,
        provider_model_id: approval.provider_model_id,
        destination: contract_destination(approval.destination_category),
        expected_size_bytes: approval.expected_size_bytes,
        licence_spdx: approval.licence_spdx,
        provenance: approval.provenance_url,
        approved_effects: approval
            .approved_effect_codes
            .iter()
            .map(|code| contract_effect_kind(code))
            .collect(),
        external_runtime_effect: approval.external_runtime_effect,
        correlation_id: approval.correlation_id,
        request_id: approval.request_id,
        decided_at_unix_ms: contract_timestamp(approval.decided_at_unix_ms)?,
    })
}

fn build_snapshot(
    job: PersistedSetupJob,
    effects: Vec<PersistedSetupEffect>,
    artifact: Option<PersistedModelArtifact>,
    events: Vec<PersistedSetupEvent>,
    request_id: RequestId,
) -> Result<SetupJobSnapshot, CoreError> {
    let correlation_id = events
        .last()
        .map(|event| event.correlation_id)
        .ok_or(CoreError::SetupPersistenceUnavailable)?;
    let effect_report = contract_effect_report(&effects);
    let cancelled_at = job
        .cancelled_at_unix_ms
        .map(contract_timestamp)
        .transpose()?;
    let cancellation_report =
        cancelled_at
            .filter(|_| job.cancellation_requested)
            .map(|observed_at_unix_ms| {
                let requested_at_unix_ms = events
                    .iter()
                    .rev()
                    .find(|event| event.kind == PersistedSetupEventKind::CancellationRequested)
                    .and_then(|event| contract_timestamp(event.occurred_at_unix_ms).ok())
                    .unwrap_or(observed_at_unix_ms);
                SetupCancellationReport {
                    requested_at_unix_ms,
                    observed_at_unix_ms,
                    effect_report: effect_report.clone().unwrap_or(SetupEffectReport {
                        effects: Vec::new(),
                    }),
                }
            });
    let attention_reason = job
        .attention_reason
        .as_deref()
        .map(contract_attention_reason);
    let error = job
        .last_safe_error_code
        .as_deref()
        .map(|code| safe_setup_error(code, correlation_id, request_id))
        .transpose()?;
    let recovery_action = attention_reason.map(recovery_for_attention);
    let plan = contract_plan(&job, artifact.as_ref())?;

    Ok(SetupJobSnapshot {
        schema_version: SETUP_WORKFLOW_SCHEMA_VERSION,
        job_id: contract_job_id(&job.plan.job_id)?,
        plan,
        state: contract_job_state(job.state),
        stage: contract_stage(job.state),
        progress: contract_progress(&job.progress),
        attention_reason,
        recovery_action,
        error,
        effect_report,
        cancellation_report,
        cancellation_requested: job.cancellation_requested,
        retry_count: job.retry_count,
        latest_event_sequence: job.last_event_sequence,
        created_at_unix_ms: contract_timestamp(job.plan.created_at_unix_ms)?,
        updated_at_unix_ms: contract_timestamp(job.updated_at_unix_ms)?,
    })
}

fn contract_plan(
    job: &PersistedSetupJob,
    artifact: Option<&PersistedModelArtifact>,
) -> Result<SetupPlan, CoreError> {
    let integrity = artifact.map_or(ModelIntegrityState::Unavailable, |value| {
        contract_integrity(value.integrity)
    });
    let verification = contract_verification_result(job, integrity)?;
    let model = ModelMetadata {
        schema_version: SETUP_WORKFLOW_SCHEMA_VERSION,
        artifact: ModelProviderArtifact {
            canonical_model_id: job.plan.canonical_model_id.clone(),
            provider_id: job.plan.provider_id.clone(),
            provider_model_id: job.plan.provider_model_id.clone(),
            source_summary: job.plan.artifact_source_summary.clone(),
        },
        display_name: job.plan.model_display_name.clone(),
        family: job.plan.model_family.clone(),
        size_class: job.plan.model_size_class,
        licence_spdx: job.plan.licence_spdx.clone(),
        provenance: job.plan.provenance_url.clone(),
        destination: contract_destination(job.plan.destination_category),
        destination_display: job.plan.destination_display.clone(),
        expected_size_bytes: job.plan.expected_size_bytes,
        measured_size_bytes: artifact.and_then(|value| value.measured_size_bytes),
        catalogue_version: job.plan.catalogue_version.clone(),
        rule_set_version: job.plan.rule_set_version.clone(),
        lifecycle_state: artifact.map_or(ModelLifecycleState::Planned, |value| {
            contract_model_lifecycle(value.lifecycle)
        }),
        verification_state: artifact.map_or(ModelVerificationState::NotStarted, |value| {
            contract_model_verification(value.verification, job)
        }),
        verification,
    };
    let components = vec![
        SetupPlanComponent {
            kind: SetupPlanComponentKind::Runtime,
            title: job.plan.runtime_display_name.clone(),
            detail: job.plan.runtime_version.as_ref().map_or_else(
                || "A compatible local runtime must remain healthy.".to_owned(),
                |version| format!("Compatible runtime version {version}."),
            ),
            required: true,
        },
        SetupPlanComponent {
            kind: SetupPlanComponentKind::Model,
            title: job.plan.model_display_name.clone(),
            detail: job.plan.artifact_source_summary.clone(),
            required: true,
        },
        SetupPlanComponent {
            kind: SetupPlanComponentKind::Storage,
            title: job.plan.destination_display.clone(),
            detail: format!(
                "Requires {} bytes plus a {} byte safety margin.",
                job.plan.expected_size_bytes, job.plan.storage_safety_margin_bytes
            ),
            required: true,
        },
        SetupPlanComponent {
            kind: SetupPlanComponentKind::Verification,
            title: "Readiness verification".to_owned(),
            detail: "Runtime health, model registration, and a bounded test inference must pass."
                .to_owned(),
            required: true,
        },
    ];
    let reasons = job
        .plan
        .notices
        .iter()
        .filter(|notice| !notice.warning)
        .map(|notice| SetupReason {
            code: match notice.code.as_str() {
                "recommendation_selected" | "recommendation_reason" => {
                    SetupReasonCode::RecommendationSelected
                }
                "existing_model_reusable" => SetupReasonCode::ExistingModelReusable,
                "acquisition_required" => SetupReasonCode::AcquisitionRequired,
                "approval_required" => SetupReasonCode::ApprovalRequired,
                "storage_verified" => SetupReasonCode::StorageVerified,
                "runtime_verified" => SetupReasonCode::RuntimeVerified,
                "registration_verified" => SetupReasonCode::RegistrationVerified,
                "readiness_verified" => SetupReasonCode::ReadinessVerified,
                _ => SetupReasonCode::Unknown,
            },
            message: notice.message.clone(),
        })
        .collect();
    let warnings = job
        .plan
        .notices
        .iter()
        .filter(|notice| notice.warning)
        .map(|notice| SetupWarning {
            code: match notice.code.as_str() {
                "external_runtime_modified" => SetupWarningCode::ExternalRuntimeModified,
                "provider_managed_storage" => SetupWarningCode::ProviderManagedStorage,
                "integrity_metadata_unavailable" => SetupWarningCode::IntegrityMetadataUnavailable,
                "cancellation_may_retain_effects" => SetupWarningCode::CancellationMayRetainEffects,
                "destination_evidence_incomplete" => {
                    SetupWarningCode::DestinationEvidenceIncomplete
                }
                _ => SetupWarningCode::Unknown,
            },
            message: notice.message.clone(),
        })
        .collect();

    Ok(SetupPlan {
        schema_version: job.plan.plan_schema_version,
        job_id: contract_job_id(&job.plan.job_id)?,
        revision: job.plan.plan_revision,
        model,
        runtime_display_name: job.plan.runtime_display_name.clone(),
        runtime_version: job.plan.runtime_version.clone(),
        resources: ResourceEstimate {
            memory: MemoryEstimate {
                required_bytes: job.plan.expected_memory_bytes,
                safety_margin_bytes: job.plan.memory_safety_margin_bytes,
                observed_total_bytes: job.plan.observed_total_memory_bytes,
                observed_available_bytes: job.plan.observed_available_memory_bytes,
            },
            storage: StorageEstimate {
                required_bytes: job.plan.expected_size_bytes,
                safety_margin_bytes: job.plan.storage_safety_margin_bytes,
                observed_free_bytes: job.plan.observed_free_storage_bytes,
            },
            planned_context_tokens: job.plan.planned_context_tokens,
            cpu_only: job.plan.cpu_only,
            gpu_memory_bytes: job.plan.gpu_memory_bytes,
            acceleration: job.plan.acceleration,
        },
        components,
        required_effects: job
            .plan
            .effect_codes
            .iter()
            .map(|code| contract_effect_kind(code))
            .collect(),
        reasons,
        warnings,
    })
}

fn contract_verification_result(
    job: &PersistedSetupJob,
    integrity: ModelIntegrityState,
) -> Result<Option<ModelVerificationResult>, CoreError> {
    let timestamps = [
        job.runtime_verified_at_unix_ms,
        job.model_available_at_unix_ms,
        job.registration_verified_at_unix_ms,
        job.inference_verified_at_unix_ms,
    ];
    let latest =
        timestamps.into_iter().flatten().max().or_else(|| {
            (integrity == ModelIntegrityState::Mismatch).then_some(job.updated_at_unix_ms)
        });
    let Some(latest) = latest else {
        return Ok(None);
    };
    Ok(Some(ModelVerificationResult {
        runtime_health_verified: job.runtime_verified_at_unix_ms.is_some(),
        model_available: job.model_available_at_unix_ms.is_some(),
        registration_verified: job.registration_verified_at_unix_ms.is_some(),
        inference_verified: job.inference_verified_at_unix_ms.is_some(),
        integrity,
        verified_at_unix_ms: contract_timestamp(latest)?,
    }))
}

fn contract_event(
    event: &PersistedSetupEvent,
    current: &SetupJobSnapshot,
    request_id: RequestId,
) -> Result<SetupJobEvent, CoreError> {
    let mut snapshot = current.clone();
    snapshot.state = contract_job_state(event.state);
    snapshot.stage = contract_stage(event.state);
    snapshot.progress = contract_progress(&event.progress);
    snapshot.latest_event_sequence = event.sequence;
    snapshot.updated_at_unix_ms = contract_timestamp(event.occurred_at_unix_ms)?;
    if event.state == PersistedSetupState::AttentionRequired {
        snapshot.attention_reason = event
            .safe_code
            .as_deref()
            .map(contract_attention_reason)
            .filter(|reason| *reason != SetupAttentionReason::Unknown)
            .or(snapshot.attention_reason);
        snapshot.recovery_action = snapshot.attention_reason.map(recovery_for_attention);
    } else {
        snapshot.attention_reason = None;
        snapshot.recovery_action = None;
    }
    snapshot.error = event
        .safe_code
        .as_deref()
        .filter(|code| code.starts_with("setup."))
        .map(|code| safe_setup_error(code, event.correlation_id, request_id))
        .transpose()?;
    if event.sequence != current.latest_event_sequence {
        snapshot.plan.model.measured_size_bytes = None;
        snapshot.plan.model.lifecycle_state = event_model_lifecycle(event.state);
        snapshot.plan.model.verification_state = event_model_verification(event.state);
        snapshot.plan.model.verification = None;
        snapshot.effect_report = None;
        snapshot.cancellation_report = None;
        snapshot.cancellation_requested =
            event.kind == PersistedSetupEventKind::CancellationRequested;
        snapshot.retry_count = 0;
    }

    Ok(SetupJobEvent {
        schema_version: SETUP_WORKFLOW_SCHEMA_VERSION,
        job_id: contract_job_id(&event.job_id)?,
        correlation_id: event.correlation_id,
        sequence: event.sequence,
        kind: contract_event_kind(event.kind, event.state),
        timestamp_unix_ms: contract_timestamp(event.occurred_at_unix_ms)?,
        terminal_state: contract_terminal_state(event.state),
        job: snapshot,
    })
}

const fn event_model_lifecycle(state: PersistedSetupState) -> ModelLifecycleState {
    match state {
        PersistedSetupState::DraftPlan
        | PersistedSetupState::AwaitingApproval
        | PersistedSetupState::Approved
        | PersistedSetupState::Preparing
        | PersistedSetupState::CheckingStorage => ModelLifecycleState::Planned,
        PersistedSetupState::Acquiring => ModelLifecycleState::Acquiring,
        PersistedSetupState::Registering | PersistedSetupState::VerifyingRuntime => {
            ModelLifecycleState::Acquired
        }
        PersistedSetupState::VerifyingModel | PersistedSetupState::RunningTestInference => {
            ModelLifecycleState::Verifying
        }
        PersistedSetupState::Ready => ModelLifecycleState::Available,
        PersistedSetupState::AttentionRequired => ModelLifecycleState::AttentionRequired,
        PersistedSetupState::Failed => ModelLifecycleState::Failed,
        PersistedSetupState::Cancelled => ModelLifecycleState::Cancelled,
        _ => ModelLifecycleState::Unknown,
    }
}

const fn event_model_verification(state: PersistedSetupState) -> ModelVerificationState {
    match state {
        PersistedSetupState::VerifyingModel => ModelVerificationState::RuntimeVerified,
        PersistedSetupState::RunningTestInference => ModelVerificationState::RegistrationVerified,
        PersistedSetupState::Ready => ModelVerificationState::Verified,
        PersistedSetupState::Failed => ModelVerificationState::Failed,
        PersistedSetupState::Cancelled => ModelVerificationState::Cancelled,
        _ => ModelVerificationState::NotStarted,
    }
}

fn contract_progress(progress: &PersistedSetupProgress) -> Option<ModelAcquisitionProgress> {
    if progress.phase.is_none()
        && progress.completed_bytes.is_none()
        && progress.total_bytes.is_none()
        && progress.basis_points.is_none()
    {
        return None;
    }
    Some(ModelAcquisitionProgress {
        phase: progress.phase.unwrap_or(ModelAcquisitionPhase::Unknown),
        completed_bytes: progress.completed_bytes,
        total_bytes: progress.total_bytes,
        progress_basis_points: progress.basis_points,
    })
}

fn contract_effect_report(effects: &[PersistedSetupEffect]) -> Option<SetupEffectReport> {
    if effects.is_empty() {
        return None;
    }
    Some(SetupEffectReport {
        effects: effects
            .iter()
            .map(|effect| {
                let kind = contract_effect_kind(&effect.effect_code);
                let disposition = contract_effect_disposition(effect.disposition);
                SetupEffect {
                    kind,
                    disposition,
                    message: effect_message(kind, disposition).to_owned(),
                }
            })
            .collect(),
    })
}

fn contract_effect_kind(code: &str) -> SetupEffectKind {
    match code {
        EFFECT_ACQUISITION => SetupEffectKind::ProviderModelAcquisition,
        EFFECT_REGISTRATION => SetupEffectKind::ProviderModelRegistration,
        EFFECT_INFERENCE => SetupEffectKind::ReadinessInference,
        EFFECT_METADATA => SetupEffectKind::MetadataPersistence,
        _ => SetupEffectKind::Unknown,
    }
}

const fn contract_effect_disposition(
    disposition: PersistedEffectDisposition,
) -> SetupEffectDisposition {
    match disposition {
        PersistedEffectDisposition::Completed => SetupEffectDisposition::Completed,
        PersistedEffectDisposition::Retained => SetupEffectDisposition::Retained,
        PersistedEffectDisposition::RolledBack => SetupEffectDisposition::RolledBack,
        PersistedEffectDisposition::Uncertain => SetupEffectDisposition::Uncertain,
        _ => SetupEffectDisposition::Unknown,
    }
}

const fn effect_message(
    kind: SetupEffectKind,
    disposition: SetupEffectDisposition,
) -> &'static str {
    match (kind, disposition) {
        (_, SetupEffectDisposition::Completed) => "The approved effect completed.",
        (_, SetupEffectDisposition::Retained) => "The approved effect remains on this computer.",
        (_, SetupEffectDisposition::RolledBack) => "The approved effect was rolled back.",
        (_, SetupEffectDisposition::Uncertain) => {
            "The provider effect could not be classified safely."
        }
        _ => "The setup effect has an unknown disposition.",
    }
}

const fn contract_job_state(state: PersistedSetupState) -> SetupJobState {
    match state {
        PersistedSetupState::DraftPlan => SetupJobState::DraftPlan,
        PersistedSetupState::AwaitingApproval => SetupJobState::AwaitingApproval,
        PersistedSetupState::Approved => SetupJobState::Approved,
        PersistedSetupState::Preparing
        | PersistedSetupState::CheckingStorage
        | PersistedSetupState::Acquiring
        | PersistedSetupState::Registering
        | PersistedSetupState::VerifyingRuntime
        | PersistedSetupState::VerifyingModel
        | PersistedSetupState::RunningTestInference => SetupJobState::Active,
        PersistedSetupState::AttentionRequired => SetupJobState::AttentionRequired,
        PersistedSetupState::Ready => SetupJobState::Ready,
        PersistedSetupState::Failed => SetupJobState::Failed,
        PersistedSetupState::Cancelled => SetupJobState::Cancelled,
        _ => SetupJobState::Unknown,
    }
}

const fn contract_stage(state: PersistedSetupState) -> SetupStage {
    match state {
        PersistedSetupState::DraftPlan => SetupStage::DraftPlan,
        PersistedSetupState::AwaitingApproval => SetupStage::AwaitingApproval,
        PersistedSetupState::Approved => SetupStage::Approved,
        PersistedSetupState::Preparing => SetupStage::Preparing,
        PersistedSetupState::CheckingStorage => SetupStage::CheckingStorage,
        PersistedSetupState::Acquiring => SetupStage::Acquiring,
        PersistedSetupState::Registering => SetupStage::Registering,
        PersistedSetupState::VerifyingRuntime => SetupStage::VerifyingRuntime,
        PersistedSetupState::VerifyingModel => SetupStage::VerifyingModel,
        PersistedSetupState::RunningTestInference => SetupStage::RunningTestInference,
        PersistedSetupState::Ready => SetupStage::Ready,
        PersistedSetupState::AttentionRequired => SetupStage::AttentionRequired,
        PersistedSetupState::Failed => SetupStage::Failed,
        PersistedSetupState::Cancelled => SetupStage::Cancelled,
        _ => SetupStage::Unknown,
    }
}

const fn contract_terminal_state(state: PersistedSetupState) -> Option<SetupJobTerminalState> {
    match state {
        PersistedSetupState::Ready => Some(SetupJobTerminalState::Ready),
        PersistedSetupState::AttentionRequired => Some(SetupJobTerminalState::AttentionRequired),
        PersistedSetupState::Failed => Some(SetupJobTerminalState::Failed),
        PersistedSetupState::Cancelled => Some(SetupJobTerminalState::Cancelled),
        _ => None,
    }
}

fn contract_event_kind(
    kind: PersistedSetupEventKind,
    state: PersistedSetupState,
) -> SetupJobEventKind {
    match state {
        PersistedSetupState::Ready => return SetupJobEventKind::Ready,
        PersistedSetupState::AttentionRequired => {
            return SetupJobEventKind::AttentionRequired;
        }
        PersistedSetupState::Failed => return SetupJobEventKind::Failed,
        PersistedSetupState::Cancelled => return SetupJobEventKind::Cancelled,
        _ => {}
    }
    match kind {
        PersistedSetupEventKind::StageChanged if state == PersistedSetupState::Preparing => {
            SetupJobEventKind::Started
        }
        PersistedSetupEventKind::Progress
        | PersistedSetupEventKind::CancellationRequested
        | PersistedSetupEventKind::Snapshot
        | PersistedSetupEventKind::StageChanged
        | PersistedSetupEventKind::Recovery => SetupJobEventKind::Progress,
        PersistedSetupEventKind::Retry => SetupJobEventKind::Started,
        _ => SetupJobEventKind::Unknown,
    }
}

fn contract_attention_reason(reason: &str) -> SetupAttentionReason {
    match reason {
        "runtime_not_installed" | "setup.runtime_not_installed" => {
            SetupAttentionReason::RuntimeNotInstalled
        }
        "runtime_consent_required" | "setup.runtime_consent_required" => {
            SetupAttentionReason::RuntimeConsentRequired
        }
        "runtime_unavailable" | "setup.runtime_unavailable" | "setup.operation_timed_out" => {
            SetupAttentionReason::RuntimeUnavailable
        }
        "privileged_runtime_installation_required" => {
            SetupAttentionReason::PrivilegedRuntimeInstallationRequired
        }
        "destination_unavailable"
        | "setup.destination_unavailable"
        | "setup.storage_unverified" => SetupAttentionReason::DestinationUnavailable,
        "insufficient_storage" | "setup.insufficient_storage" => {
            SetupAttentionReason::InsufficientStorage
        }
        "acquisition_interrupted"
        | "provider_mapping_changed"
        | "setup.acquisition_interrupted"
        | "setup.acquisition_timed_out"
        | "setup.progress_persistence_failed"
        | "setup.provider_mapping_changed" => SetupAttentionReason::AcquisitionInterrupted,
        "registration_unverified" | "setup.registration_unverified" => {
            SetupAttentionReason::RegistrationUnverified
        }
        "model_unavailable" | "setup.model_unavailable" => SetupAttentionReason::ModelUnavailable,
        "integrity_mismatch" | "setup.integrity_mismatch" => {
            SetupAttentionReason::IntegrityMismatch
        }
        "readiness_timed_out" | "setup.readiness_timed_out" => {
            SetupAttentionReason::ReadinessTimedOut
        }
        "readiness_failed" | "setup.readiness_failed" => SetupAttentionReason::ReadinessFailed,
        "recovery_required" | "setup.recovery_required" => SetupAttentionReason::RecoveryRequired,
        _ => SetupAttentionReason::Unknown,
    }
}

const fn recovery_for_attention(reason: SetupAttentionReason) -> SetupRecoveryAction {
    match reason {
        SetupAttentionReason::RuntimeNotInstalled
        | SetupAttentionReason::PrivilegedRuntimeInstallationRequired => {
            SetupRecoveryAction::CheckPrerequisites
        }
        SetupAttentionReason::RuntimeConsentRequired => SetupRecoveryAction::ReviewApproval,
        SetupAttentionReason::RuntimeUnavailable => SetupRecoveryAction::RestoreRuntime,
        SetupAttentionReason::DestinationUnavailable => SetupRecoveryAction::RestoreDestination,
        SetupAttentionReason::InsufficientStorage => SetupRecoveryAction::FreeStorage,
        SetupAttentionReason::IntegrityMismatch => SetupRecoveryAction::ContactSupport,
        SetupAttentionReason::AcquisitionInterrupted
        | SetupAttentionReason::RegistrationUnverified
        | SetupAttentionReason::ModelUnavailable
        | SetupAttentionReason::ReadinessTimedOut
        | SetupAttentionReason::ReadinessFailed
        | SetupAttentionReason::RecoveryRequired => SetupRecoveryAction::Retry,
        _ => SetupRecoveryAction::ContactSupport,
    }
}

const fn contract_model_lifecycle(lifecycle: PersistedModelLifecycle) -> ModelLifecycleState {
    match lifecycle {
        PersistedModelLifecycle::Planned => ModelLifecycleState::Planned,
        PersistedModelLifecycle::Acquiring => ModelLifecycleState::Acquiring,
        PersistedModelLifecycle::AcquiredUnverified => ModelLifecycleState::Acquired,
        PersistedModelLifecycle::Verifying => ModelLifecycleState::Verifying,
        PersistedModelLifecycle::Available => ModelLifecycleState::Available,
        PersistedModelLifecycle::RetainedUnverified => ModelLifecycleState::AttentionRequired,
        PersistedModelLifecycle::Rejected | PersistedModelLifecycle::Failed => {
            ModelLifecycleState::Failed
        }
        _ => ModelLifecycleState::Unknown,
    }
}

const fn contract_model_verification(
    verification: PersistedModelVerification,
    job: &PersistedSetupJob,
) -> ModelVerificationState {
    match verification {
        PersistedModelVerification::NotStarted => ModelVerificationState::NotStarted,
        PersistedModelVerification::Pending => {
            if job.registration_verified_at_unix_ms.is_some() {
                ModelVerificationState::RegistrationVerified
            } else if job.model_available_at_unix_ms.is_some() {
                ModelVerificationState::AvailabilityVerified
            } else if job.runtime_verified_at_unix_ms.is_some() {
                ModelVerificationState::RuntimeVerified
            } else {
                ModelVerificationState::NotStarted
            }
        }
        PersistedModelVerification::Verified => ModelVerificationState::Verified,
        PersistedModelVerification::Failed => ModelVerificationState::Failed,
        PersistedModelVerification::Unknown => ModelVerificationState::Unknown,
        _ => ModelVerificationState::Unknown,
    }
}

const fn contract_integrity(integrity: PersistedModelIntegrity) -> ModelIntegrityState {
    match integrity {
        PersistedModelIntegrity::Unavailable => ModelIntegrityState::Unavailable,
        PersistedModelIntegrity::ProviderReported => ModelIntegrityState::ProviderReported,
        PersistedModelIntegrity::Verified => ModelIntegrityState::Verified,
        PersistedModelIntegrity::Mismatch => ModelIntegrityState::Mismatch,
        PersistedModelIntegrity::Unknown => ModelIntegrityState::Unknown,
        _ => ModelIntegrityState::Unknown,
    }
}

const fn persisted_integrity(integrity: ModelIntegrityState) -> PersistedModelIntegrity {
    match integrity {
        ModelIntegrityState::Unavailable => PersistedModelIntegrity::Unavailable,
        ModelIntegrityState::ProviderReported => PersistedModelIntegrity::ProviderReported,
        ModelIntegrityState::Verified => PersistedModelIntegrity::Verified,
        ModelIntegrityState::Mismatch => PersistedModelIntegrity::Mismatch,
        ModelIntegrityState::Unknown => PersistedModelIntegrity::Unknown,
        _ => PersistedModelIntegrity::Unknown,
    }
}

fn prepared_matches_job(prepared: &RuntimeModelAcquisitionPlan, job: &PersistedSetupJob) -> bool {
    prepared.artifact.canonical_model_id == job.plan.canonical_model_id
        && prepared.artifact.provider_id == job.plan.provider_id
        && prepared.artifact.provider_model_id == job.plan.provider_model_id
        && prepared.artifact.source_summary == job.plan.artifact_source_summary
        && prepared.destination == contract_destination(job.plan.destination_category)
        && prepared.destination_display == job.plan.destination_display
}

fn storage_checkpoint_due(
    progress: &ModelAcquisitionProgress,
    last_bytes: u64,
    last_basis_points: u16,
    last_phase: Option<ModelAcquisitionPhase>,
) -> bool {
    last_phase != Some(progress.phase)
        || progress
            .completed_bytes
            .is_some_and(|bytes| bytes.saturating_sub(last_bytes) >= STORAGE_CHECKPOINT_BYTES)
        || progress.progress_basis_points.is_some_and(|basis_points| {
            basis_points.saturating_sub(last_basis_points) >= STORAGE_CHECKPOINT_BASIS_POINTS
        })
}

const fn runtime_attention(
    error: &RuntimeError,
    state: PersistedSetupState,
) -> (&'static str, &'static str) {
    match error {
        RuntimeError::NotInstalled => ("runtime_not_installed", "setup.runtime_not_installed"),
        RuntimeError::ConsentRequired | RuntimeError::OwnershipConflict => {
            ("runtime_consent_required", "setup.runtime_consent_required")
        }
        RuntimeError::ModelStorageExhausted => {
            ("insufficient_storage", "setup.insufficient_storage")
        }
        RuntimeError::ModelStorageUnavailable => {
            ("destination_unavailable", "setup.destination_unavailable")
        }
        RuntimeError::ModelUnavailable => ("model_unavailable", "setup.model_unavailable"),
        RuntimeError::ModelRegistrationFailed => {
            ("registration_unverified", "setup.registration_unverified")
        }
        RuntimeError::ModelIntegrityFailed => ("integrity_mismatch", "setup.integrity_mismatch"),
        RuntimeError::TimedOut if matches!(state, PersistedSetupState::RunningTestInference) => {
            ("readiness_timed_out", "setup.readiness_timed_out")
        }
        RuntimeError::TimedOut if matches!(state, PersistedSetupState::Acquiring) => {
            ("acquisition_interrupted", "setup.acquisition_timed_out")
        }
        RuntimeError::TimedOut => ("runtime_unavailable", "setup.operation_timed_out"),
        RuntimeError::ReadinessInferenceFailed => ("readiness_failed", "setup.readiness_failed"),
        RuntimeError::InvalidResponse | RuntimeError::OutputLimit
            if matches!(state, PersistedSetupState::RunningTestInference) =>
        {
            ("readiness_failed", "setup.readiness_failed")
        }
        RuntimeError::ModelAcquisitionFailed
        | RuntimeError::Cancelled
        | RuntimeError::InvalidResponse
        | RuntimeError::OutputLimit => ("acquisition_interrupted", "setup.acquisition_interrupted"),
        _ => ("runtime_unavailable", "setup.runtime_unavailable"),
    }
}

fn safe_setup_error(
    code: &str,
    correlation_id: CorrelationId,
    request_id: RequestId,
) -> Result<SafeErrorPayload, CoreError> {
    let (category, message, action, recovery) = match code {
        "setup.insufficient_storage" => (
            ErrorCategory::ResourceExhausted,
            "There is not enough verified storage for the selected model.",
            RecoveryAction::CheckPrerequisites,
            "Free storage while preserving the required safety margin, then retry.",
        ),
        "setup.integrity_mismatch" => (
            ErrorCategory::IntegrityFailure,
            "The selected model did not pass available integrity checks.",
            RecoveryAction::ContactSupport,
            "Do not use the retained artifact. Review sanitized diagnostics.",
        ),
        "setup.retry_limit_reached" => (
            ErrorCategory::ResourceExhausted,
            "This setup plan reached its bounded retry limit.",
            RecoveryAction::ContactSupport,
            "Create and approve a fresh plan. If setup still fails, review sanitized diagnostics.",
        ),
        "setup.operation_timed_out"
        | "setup.readiness_timed_out"
        | "setup.acquisition_timed_out" => (
            ErrorCategory::TimedOut,
            "A bounded setup operation did not finish in time.",
            RecoveryAction::Retry,
            "Check runtime and storage status, then retry.",
        ),
        "setup.provider_mapping_changed" | "setup.recovery_required" => (
            ErrorCategory::Conflict,
            "Setup evidence changed and requires a fresh explicit action.",
            RecoveryAction::Retry,
            "Refresh the current setup state before continuing.",
        ),
        "setup.readiness_failed" | "setup.registration_unverified" => (
            ErrorCategory::Degraded,
            "The local model could not be verified as ready.",
            RecoveryAction::Retry,
            "Check the local runtime status, then retry verification.",
        ),
        _ => (
            ErrorCategory::Unavailable,
            "Model setup needs attention before it can continue.",
            RecoveryAction::Retry,
            "Review the current setup state and retry the offered action.",
        ),
    };
    Ok(SafeErrorPayload::new(
        category,
        code,
        message,
        RecoveryGuidance {
            action,
            message: recovery.to_owned(),
        },
        correlation_id,
        request_id,
    ))
}

fn contract_timestamp(value: i64) -> Result<u64, CoreError> {
    u64::try_from(value).map_err(|_| CoreError::SetupPersistenceUnavailable)
}

fn basic_transition(
    job: &PersistedSetupJob,
    next_state: PersistedSetupState,
    correlation_id: CorrelationId,
) -> Result<PersistedSetupTransition, CoreError> {
    let next_stage = stage_for(next_state);
    validate_transition(job.state, next_state, next_stage)?;
    Ok(PersistedSetupTransition {
        job_id: job.plan.job_id.clone(),
        expected_state: job.state,
        expected_revision: job.revision,
        next_state,
        next_stage,
        progress: None,
        clear_progress: false,
        safe_error_code: None,
        attention_reason: None,
        runtime_verified_at_unix_ms: None,
        model_available_at_unix_ms: None,
        registration_verified_at_unix_ms: None,
        inference_verified_at_unix_ms: None,
        clear_verification: false,
        clear_cancellation: false,
        increment_retry: false,
        artifact_update: None,
        effects: Vec::new(),
        event_kind: PersistedSetupEventKind::StageChanged,
        correlation_id,
        occurred_at_unix_ms: unix_timestamp_millis(),
    })
}

fn attempt_start_transition(
    job: &PersistedSetupJob,
    correlation_id: CorrelationId,
    retry: bool,
) -> Result<PersistedSetupTransition, CoreError> {
    let mut transition = basic_transition(job, PersistedSetupState::Preparing, correlation_id)?;
    transition.clear_progress = retry;
    transition.clear_verification = retry;
    transition.clear_cancellation = retry;
    transition.increment_retry = retry;
    transition.event_kind = if retry {
        PersistedSetupEventKind::Retry
    } else {
        PersistedSetupEventKind::StageChanged
    };
    if retry {
        transition.artifact_update = Some(PersistedArtifactUpdate {
            lifecycle: PersistedModelLifecycle::Planned,
            verification: PersistedModelVerification::NotStarted,
            integrity: PersistedModelIntegrity::Unavailable,
            measured_size_bytes: None,
            registered_at_unix_ms: None,
            verified_at_unix_ms: None,
        });
    }
    Ok(transition)
}

fn cancelled_transition(
    job: &PersistedSetupJob,
    correlation_id: CorrelationId,
    effect: Option<(PersistedEffectDisposition, &'static str)>,
) -> Result<PersistedSetupTransition, CoreError> {
    let mut transition = basic_transition(job, PersistedSetupState::Cancelled, correlation_id)?;
    transition.event_kind = PersistedSetupEventKind::Terminal;
    let retained_effect_possible = effect.is_some();
    if retained_effect_possible {
        transition.artifact_update = Some(PersistedArtifactUpdate {
            lifecycle: PersistedModelLifecycle::RetainedUnverified,
            verification: PersistedModelVerification::Unknown,
            integrity: PersistedModelIntegrity::Unknown,
            measured_size_bytes: None,
            registered_at_unix_ms: None,
            verified_at_unix_ms: None,
        });
    }
    if let Some((disposition, effect_code)) = effect {
        transition.effects.push(PersistedSetupEffectInput {
            disposition,
            effect_code: effect_code.to_owned(),
            size_bytes: None,
        });
    }
    Ok(transition)
}

const fn pending_effect_for_state(state: PersistedSetupState) -> Option<&'static str> {
    match state {
        PersistedSetupState::Acquiring => Some(EFFECT_ACQUISITION),
        PersistedSetupState::RunningTestInference => Some(EFFECT_INFERENCE),
        _ => None,
    }
}

const fn acquisition_may_have_started(state: PersistedSetupState) -> bool {
    matches!(
        state,
        PersistedSetupState::Acquiring
            | PersistedSetupState::Registering
            | PersistedSetupState::VerifyingRuntime
            | PersistedSetupState::VerifyingModel
            | PersistedSetupState::RunningTestInference
    )
}

fn non_retriable_failure(job: &PersistedSetupJob) -> bool {
    job.state == PersistedSetupState::Failed
        && matches!(
            job.last_safe_error_code.as_deref(),
            Some(
                "setup.integrity_mismatch"
                    | "setup.provider_contract_unsupported"
                    | "setup.retry_limit_reached"
            )
        )
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use gixgiz_contracts::{
        CandidateModelId, CatalogueVersion, ModelSizeClass, ProviderRegistrationResult,
        RUNTIME_REPORT_SCHEMA_VERSION, RuleSetVersion, RuntimeCapabilityAvailability,
        RuntimeCapabilityDescriptor, RuntimeCapabilityKind, RuntimeConsentDecision,
        RuntimeDisplayName, RuntimeEndpointSafety, RuntimeModelInventory, RuntimeOperationKind,
        RuntimeProviderId, RuntimeProviderModelId,
    };
    use gixgiz_persistence::{DataRoot, PersistedSetupStage};
    use gixgiz_runtime::{
        ChatDeltaSender, ModelProgressSender, RuntimeChatProvider, RuntimeChatRequest,
        RuntimeDetector, RuntimeFuture, RuntimeGenerationResult, RuntimeLifecycle,
        RuntimeModelAcquisitionResult, RuntimeModelAcquisitionStatus, RuntimeModelInspection,
        RuntimeModelInventoryProvider, RuntimeModelSetupProvider, RuntimeObservation,
        RuntimeReadinessInferenceResult, RuntimeStoragePreflight,
        testing::{FakeRuntimeCall, FakeRuntimeProvider},
    };

    use super::*;

    fn artifact(provider_id: &RuntimeProviderId) -> ModelProviderArtifact {
        ModelProviderArtifact {
            canonical_model_id: CandidateModelId::new("test-model-compact"),
            provider_id: provider_id.clone(),
            provider_model_id: RuntimeProviderModelId::new("provider-test-model:latest"),
            source_summary: "Repository-owned provider mapping".to_owned(),
        }
    }

    fn observation(provider_id: &RuntimeProviderId) -> RuntimeObservation {
        RuntimeObservation {
            provider_id: provider_id.clone(),
            display_name: RuntimeDisplayName::new("Test runtime"),
            state: RuntimeState::Ready,
            endpoint_safety: RuntimeEndpointSafety::LoopbackVerified,
            version: None,
            capabilities: [
                RuntimeCapabilityKind::Detection,
                RuntimeCapabilityKind::Health,
                RuntimeCapabilityKind::Version,
                RuntimeCapabilityKind::ModelInventory,
                RuntimeCapabilityKind::ModelAcquisitionPreparation,
                RuntimeCapabilityKind::ModelStoragePreflight,
                RuntimeCapabilityKind::ModelAcquisition,
                RuntimeCapabilityKind::ModelRegistration,
                RuntimeCapabilityKind::ReadinessInference,
            ]
            .into_iter()
            .map(|kind| RuntimeCapabilityDescriptor {
                kind,
                availability: RuntimeCapabilityAvailability::Available,
                reason: None,
            })
            .collect(),
            reasons: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn available_storage() -> RuntimeStoragePreflight {
        RuntimeStoragePreflight {
            destination: SetupDestinationCategory::ProviderManaged,
            destination_display: "Provider-managed model storage".to_owned(),
            availability: RuntimeStorageAvailability::Available,
            required_bytes: 1_024,
            safety_margin_bytes: 512,
            available_bytes: Some(1_536),
            checked_at_unix_ms: 10,
        }
    }

    fn unavailable_inspection() -> RuntimeModelInspection {
        model_inspection(
            false,
            ProviderRegistrationState::NotRegistered,
            ModelIntegrityState::Unavailable,
        )
    }

    fn model_inspection(
        available: bool,
        state: ProviderRegistrationState,
        integrity: ModelIntegrityState,
    ) -> RuntimeModelInspection {
        let provider_id = RuntimeProviderId::new("test.runtime");
        RuntimeModelInspection {
            artifact: artifact(&provider_id),
            available,
            registration: ProviderRegistrationResult {
                state,
                measured_size_bytes: available.then_some(1_000),
                verified_at_unix_ms: 30,
            },
            integrity,
        }
    }

    fn fake_provider(progress: Vec<ModelAcquisitionProgress>) -> FakeRuntimeProvider {
        fake_provider_with_readiness(
            progress,
            Ok(RuntimeReadinessInferenceResult {
                ready: true,
                completed_at_unix_ms: 40,
            }),
        )
    }

    fn fake_provider_with_readiness(
        progress: Vec<ModelAcquisitionProgress>,
        readiness: Result<RuntimeReadinessInferenceResult, RuntimeError>,
    ) -> FakeRuntimeProvider {
        let provider_id = RuntimeProviderId::new("test.runtime");
        let provider_artifact = artifact(&provider_id);
        let observed = observation(&provider_id);
        FakeRuntimeProvider::new(
            provider_id.clone(),
            Ok(observed.clone()),
            Ok(observed),
            Ok(RuntimeModelInventory {
                schema_version: RUNTIME_REPORT_SCHEMA_VERSION,
                provider_id,
                models: Vec::new(),
                truncated: false,
                collected_at_unix_ms: 1,
            }),
        )
        .with_model_setup_results(
            Ok(RuntimeModelAcquisitionPlan {
                artifact: provider_artifact.clone(),
                destination: SetupDestinationCategory::ProviderManaged,
                destination_display: "Provider-managed model storage".to_owned(),
                cancellation: RuntimeCancellationSemantics::ConnectionAbortMayRetainEffects,
            }),
            Ok(available_storage()),
            Ok(RuntimeModelAcquisitionResult {
                artifact: provider_artifact.clone(),
                status: RuntimeModelAcquisitionStatus::Acquired,
                measured_size_bytes: Some(1_000),
                integrity: ModelIntegrityState::ProviderReported,
                completed_at_unix_ms: 20,
            }),
            Ok(RuntimeModelInspection {
                artifact: provider_artifact,
                available: true,
                registration: ProviderRegistrationResult {
                    state: ProviderRegistrationState::Registered,
                    measured_size_bytes: Some(1_000),
                    verified_at_unix_ms: 30,
                },
                integrity: ModelIntegrityState::ProviderReported,
            }),
            readiness,
        )
        .with_model_progress(progress)
    }

    fn persisted_plan(job_id: SetupJobId) -> PersistedSetupPlan {
        let provider_id = RuntimeProviderId::new("test.runtime");
        let provider_artifact = artifact(&provider_id);
        let capability = CapabilityEngine::v0_1();
        let (catalogue_version, rule_set_version) = capability.setup_versions();
        PersistedSetupPlan {
            job_id: persistence_job_id(job_id).expect("test job id is valid"),
            plan_schema_version: SETUP_WORKFLOW_SCHEMA_VERSION,
            plan_revision: 1,
            canonical_model_id: provider_artifact.canonical_model_id,
            model_display_name: "Test model".to_owned(),
            model_family: "test-family".to_owned(),
            model_size_class: ModelSizeClass::Compact,
            licence_spdx: "Apache-2.0".to_owned(),
            provenance_url: "https://example.com/model".to_owned(),
            provider_id,
            provider_model_id: provider_artifact.provider_model_id,
            artifact_source_summary: provider_artifact.source_summary,
            destination_display: "Provider-managed model storage".to_owned(),
            runtime_display_name: "Test runtime".to_owned(),
            runtime_version: None,
            catalogue_version: catalogue_version.clone(),
            rule_set_version: rule_set_version.clone(),
            destination_category: PersistedDestinationCategory::ProviderManaged,
            expected_size_bytes: 1_024,
            expected_memory_bytes: 2_048,
            memory_safety_margin_bytes: 512,
            storage_safety_margin_bytes: 512,
            planned_context_tokens: 512,
            cpu_only: true,
            gpu_memory_bytes: None,
            observed_total_memory_bytes: Some(8_192),
            observed_available_memory_bytes: Some(4_096),
            observed_free_storage_bytes: Some(4_096),
            acceleration: None,
            external_runtime_effect: true,
            notices: vec![PersistedSetupNotice {
                warning: false,
                code: "recommendation_selected".to_owned(),
                message: "The deterministic test plan selected this model.".to_owned(),
            }],
            effect_codes: planned_effect_codes(),
            created_at_unix_ms: 1,
        }
    }

    fn service(
        provider: Arc<dyn RuntimeProvider>,
    ) -> (tempfile::TempDir, Persistence, SetupService) {
        let temporary = tempfile::tempdir().expect("temporary directory is available");
        let root = DataRoot::from_override(temporary.path().join("root"))
            .expect("temporary data root initializes");
        let persistence = Persistence::open(root).expect("temporary persistence opens");
        let service = SetupService::with_persistence(provider, &persistence);
        (temporary, persistence, service)
    }

    async fn approve_runtime_reuse(service: &SetupService) {
        service
            .runtime
            .set_reuse_consent(
                service.provider.provider_id(),
                RuntimeConsentDecision::ApproveReuse,
                runtime_context(
                    CorrelationId::new(),
                    RequestId::new(),
                    RuntimeCancellationToken::new(),
                    Duration::from_secs(1),
                ),
            )
            .await
            .expect("fake runtime reuse is approved");
    }

    async fn persist_and_approve(service: &SetupService) -> SetupJobId {
        let job_id = SetupJobId::new();
        service
            .repository
            .create(&persisted_plan(job_id), CorrelationId::new())
            .expect("test setup plan persists");
        service
            .decide_approval(SetupApprovalRequest {
                job_id,
                plan_revision: 1,
                decision: SetupApprovalDecision::Approve,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("exact plan approval persists");
        job_id
    }

    async fn wait_for_outcome(service: &SetupService, job_id: SetupJobId) -> SetupJobSnapshot {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let snapshot = service
                    .status(SetupJobStatusRequest {
                        job_id,
                        correlation_id: CorrelationId::new(),
                        request_id: RequestId::new(),
                    })
                    .await
                    .expect("setup status loads")
                    .job;
                if matches!(
                    snapshot.state,
                    SetupJobState::Ready
                        | SetupJobState::AttentionRequired
                        | SetupJobState::Failed
                        | SetupJobState::Cancelled
                ) {
                    return snapshot;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("fake setup reaches a bounded outcome")
    }

    async fn wait_for_inactive(service: &SetupService, job_id: SetupJobId) {
        tokio::time::timeout(Duration::from_secs(1), async {
            while service.is_active(job_id).expect("active registry reads") {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("setup attempt releases its active admission");
    }

    #[tokio::test]
    async fn exact_approval_runs_every_readiness_gate_before_ready() {
        let provider = Arc::new(SequencedStorageProvider {
            inner: fake_provider(Vec::new()),
            preflights: Mutex::new(VecDeque::new()),
            fallback: available_storage(),
            inspections: Mutex::new(VecDeque::from([unavailable_inspection()])),
            storage_requests: Mutex::new(Vec::new()),
        });
        let (_temporary, persistence, service) = service(provider.clone());
        approve_runtime_reuse(&service).await;
        let job_id = persist_and_approve(&service).await;

        service
            .start_job(SetupJobStartRequest {
                job_id,
                plan_revision: 1,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("approved setup starts");
        let snapshot = wait_for_outcome(&service, job_id).await;

        assert_eq!(snapshot.state, SetupJobState::Ready, "{snapshot:#?}");
        assert_eq!(snapshot.stage, SetupStage::Ready);
        assert_eq!(
            snapshot.plan.model.lifecycle_state,
            ModelLifecycleState::Available
        );
        assert_eq!(
            snapshot.plan.model.verification_state,
            ModelVerificationState::Verified
        );
        assert!(
            snapshot
                .plan
                .model
                .verification
                .as_ref()
                .is_some_and(|result| {
                    result.runtime_health_verified
                        && result.model_available
                        && result.registration_verified
                        && result.inference_verified
                })
        );
        let calls = provider.inner.calls();
        let inspection = calls
            .iter()
            .position(|call| *call == FakeRuntimeCall::InspectModel)
            .expect("model inspection ran");
        let inference = calls
            .iter()
            .position(|call| *call == FakeRuntimeCall::RunReadinessInference)
            .expect("readiness inference ran");
        assert!(inspection < inference);
        assert!(calls.contains(&FakeRuntimeCall::AcquireModel));
        assert_eq!(
            provider
                .storage_requests
                .lock()
                .expect("storage requests lock")
                .as_slice(),
            &[1_024, 0, 0]
        );
        let stored = persistence
            .setup_jobs()
            .artifact(
                &snapshot.plan.model.artifact.canonical_model_id,
                &snapshot.plan.model.artifact.provider_id,
            )
            .expect("artifact loads")
            .expect("artifact exists");
        assert_eq!(stored.lifecycle, PersistedModelLifecycle::Available);
        assert_eq!(stored.verification, PersistedModelVerification::Verified);
        assert!(matches!(
            stored.integrity,
            PersistedModelIntegrity::ProviderReported | PersistedModelIntegrity::Verified
        ));

        let history = service
            .events_after(SetupJobEventsRequest {
                job_id,
                after_sequence: 0,
                limit: EVENT_PAGE_MAX,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("durable event history replays");
        let early = history
            .events
            .iter()
            .find(|event| event.job.state != SetupJobState::Ready)
            .expect("history contains a pre-ready event");
        assert_ne!(
            early.job.plan.model.lifecycle_state,
            ModelLifecycleState::Available
        );
        assert_ne!(
            early.job.plan.model.verification_state,
            ModelVerificationState::Verified
        );
        assert!(early.job.plan.model.verification.is_none());
        assert!(
            history
                .events
                .iter()
                .all(|event| event.kind != SetupJobEventKind::Unknown)
        );
    }

    #[tokio::test]
    async fn durable_denial_cannot_start_or_retry_provider_work() {
        let provider = Arc::new(fake_provider(Vec::new()));
        let (_temporary, _persistence, service) = service(provider.clone());
        let job_id = SetupJobId::new();
        service
            .repository
            .create(&persisted_plan(job_id), CorrelationId::new())
            .expect("test setup plan persists");
        let denied = service
            .decide_approval(SetupApprovalRequest {
                job_id,
                plan_revision: 1,
                decision: SetupApprovalDecision::Deny,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("denial persists");

        assert_eq!(denied.job.state, SetupJobState::Cancelled);
        assert_eq!(
            service
                .retry_job(SetupJobRetryRequest {
                    job_id,
                    plan_revision: 1,
                    correlation_id: CorrelationId::new(),
                    request_id: RequestId::new(),
                })
                .await,
            Err(CoreError::SetupApprovalRequired)
        );
        assert!(!provider.calls().contains(&FakeRuntimeCall::AcquireModel));
    }

    #[tokio::test]
    async fn cancellation_before_acquisition_reports_no_retained_provider_effect() {
        let provider = Arc::new(fake_provider(Vec::new()));
        let (_temporary, _persistence, service) = service(provider);
        let job_id = persist_and_approve(&service).await;

        let response = service
            .cancel_job(SetupJobCancelRequest {
                job_id,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("queued setup cancels");

        assert_eq!(response.job.state, SetupJobState::Cancelled);
        assert!(response.job.effect_report.is_none());
        assert_eq!(
            response.job.plan.model.lifecycle_state,
            ModelLifecycleState::Planned
        );
    }

    #[tokio::test]
    async fn cancelling_a_new_plan_does_not_erase_shared_available_artifact_evidence() {
        let provider = Arc::new(SequencedStorageProvider {
            inner: fake_provider(Vec::new()),
            preflights: Mutex::new(VecDeque::new()),
            fallback: available_storage(),
            inspections: Mutex::new(VecDeque::from([unavailable_inspection()])),
            storage_requests: Mutex::new(Vec::new()),
        });
        let (_temporary, _persistence, service) = service(provider);
        approve_runtime_reuse(&service).await;
        let completed_job_id = persist_and_approve(&service).await;
        service
            .start_job(SetupJobStartRequest {
                job_id: completed_job_id,
                plan_revision: 1,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("first setup starts");
        assert_eq!(
            wait_for_outcome(&service, completed_job_id).await.state,
            SetupJobState::Ready
        );

        let review_job_id = persist_and_approve(&service).await;
        let cancelled = service
            .cancel_job(SetupJobCancelRequest {
                job_id: review_job_id,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("new review cancels without provider work")
            .job;

        assert_eq!(cancelled.state, SetupJobState::Cancelled);
        assert_eq!(
            cancelled.plan.model.lifecycle_state,
            ModelLifecycleState::Available
        );
        assert_eq!(
            cancelled.plan.model.verification_state,
            ModelVerificationState::Verified
        );
    }

    #[test]
    fn retry_transition_clears_every_prior_verification_field() {
        let mut job = PersistedSetupJob {
            plan: persisted_plan(SetupJobId::new()),
            state: PersistedSetupState::AttentionRequired,
            stage: PersistedSetupStage::Attention,
            progress: PersistedSetupProgress {
                phase: Some(ModelAcquisitionPhase::Completed),
                completed_bytes: Some(1_000),
                total_bytes: Some(1_000),
                basis_points: Some(10_000),
            },
            cancellation_requested: true,
            retry_count: 2,
            last_safe_error_code: Some("setup.readiness_failed".to_owned()),
            attention_reason: Some("readiness_failed".to_owned()),
            runtime_verified_at_unix_ms: Some(10),
            model_available_at_unix_ms: Some(20),
            registration_verified_at_unix_ms: Some(30),
            inference_verified_at_unix_ms: Some(40),
            approved_at_unix_ms: Some(5),
            cancelled_at_unix_ms: None,
            last_event_sequence: 8,
            revision: 9,
            updated_at_unix_ms: 50,
        };
        let transition = attempt_start_transition(&job, CorrelationId::new(), true)
            .expect("attention state can retry");

        assert!(transition.clear_progress);
        assert!(transition.clear_verification);
        assert!(transition.clear_cancellation);
        assert!(transition.increment_retry);
        assert_eq!(transition.event_kind, PersistedSetupEventKind::Retry);
        assert!(transition.artifact_update.as_ref().is_some_and(|artifact| {
            artifact.lifecycle == PersistedModelLifecycle::Planned
                && artifact.verification == PersistedModelVerification::NotStarted
                && artifact.integrity == PersistedModelIntegrity::Unavailable
                && artifact.registered_at_unix_ms.is_none()
                && artifact.verified_at_unix_ms.is_none()
        }));

        job.state = PersistedSetupState::Ready;
        assert_eq!(
            attempt_start_transition(&job, CorrelationId::new(), true),
            Err(CoreError::SetupStateConflict)
        );
    }

    #[test]
    fn progress_thresholds_bound_storage_rechecks() {
        let progress = ModelAcquisitionProgress {
            phase: ModelAcquisitionPhase::Transferring,
            completed_bytes: Some(512),
            total_bytes: Some(1_024),
            progress_basis_points: Some(5_000),
        };
        assert!(storage_checkpoint_due(&progress, 0, 0, None));
        assert!(!storage_checkpoint_due(
            &progress,
            STORAGE_CHECKPOINT_BYTES,
            5_000,
            Some(ModelAcquisitionPhase::Transferring),
        ));
        assert!(storage_checkpoint_due(
            &ModelAcquisitionProgress {
                phase: ModelAcquisitionPhase::Verifying,
                ..progress
            },
            STORAGE_CHECKPOINT_BYTES,
            STORAGE_CHECKPOINT_BASIS_POINTS,
            Some(ModelAcquisitionPhase::Transferring),
        ));
    }

    #[test]
    fn approved_size_is_authoritative_for_progress_and_provider_overruns_fail_closed() {
        let normalized = authoritative_progress(
            1_024,
            ModelAcquisitionProgress {
                phase: ModelAcquisitionPhase::Transferring,
                completed_bytes: Some(256),
                total_bytes: Some(512),
                progress_basis_points: None,
            },
        )
        .expect("bounded provider progress is accepted");
        assert_eq!(normalized.completed_bytes, Some(256));
        assert_eq!(normalized.total_bytes, Some(1_024));
        assert_eq!(normalized.progress_basis_points, Some(2_500));

        for progress in [
            ModelAcquisitionProgress {
                phase: ModelAcquisitionPhase::Transferring,
                completed_bytes: Some(1_025),
                total_bytes: Some(1_024),
                progress_basis_points: None,
            },
            ModelAcquisitionProgress {
                phase: ModelAcquisitionPhase::Transferring,
                completed_bytes: Some(1_024),
                total_bytes: Some(1_025),
                progress_basis_points: None,
            },
        ] {
            assert_eq!(
                authoritative_progress(1_024, progress),
                Err(RuntimeError::InvalidResponse)
            );
        }
    }

    #[test]
    fn provider_summaries_are_bounded_and_cannot_expose_paths() {
        assert!(safe_provider_summary("Provider-managed model storage"));
        assert!(!safe_provider_summary("C:\\Users\\private\\models"));
        assert!(!safe_provider_summary("/home/private/models"));
        assert!(!safe_provider_summary(
            &"x".repeat(SAFE_PROVIDER_SUMMARY_BYTES_MAX + 1)
        ));
        assert!(!safe_provider_summary("private\npath"));
    }

    #[test]
    fn every_current_persisted_event_kind_has_a_stable_contract_mapping() {
        let cases = [
            (
                PersistedSetupEventKind::Snapshot,
                PersistedSetupState::AwaitingApproval,
                SetupJobEventKind::Progress,
            ),
            (
                PersistedSetupEventKind::StageChanged,
                PersistedSetupState::Preparing,
                SetupJobEventKind::Started,
            ),
            (
                PersistedSetupEventKind::StageChanged,
                PersistedSetupState::CheckingStorage,
                SetupJobEventKind::Progress,
            ),
            (
                PersistedSetupEventKind::Progress,
                PersistedSetupState::Acquiring,
                SetupJobEventKind::Progress,
            ),
            (
                PersistedSetupEventKind::CancellationRequested,
                PersistedSetupState::Acquiring,
                SetupJobEventKind::Progress,
            ),
            (
                PersistedSetupEventKind::Retry,
                PersistedSetupState::Preparing,
                SetupJobEventKind::Started,
            ),
            (
                PersistedSetupEventKind::Recovery,
                PersistedSetupState::Preparing,
                SetupJobEventKind::Progress,
            ),
            (
                PersistedSetupEventKind::Terminal,
                PersistedSetupState::Ready,
                SetupJobEventKind::Ready,
            ),
        ];

        for (kind, state, expected) in cases {
            assert_eq!(contract_event_kind(kind, state), expected);
        }
    }

    #[test]
    fn runtime_errors_are_classified_by_the_active_setup_stage() {
        assert_eq!(
            runtime_attention(&RuntimeError::TimedOut, PersistedSetupState::Acquiring),
            ("acquisition_interrupted", "setup.acquisition_timed_out")
        );
        assert_eq!(
            runtime_attention(
                &RuntimeError::TimedOut,
                PersistedSetupState::RunningTestInference,
            ),
            ("readiness_timed_out", "setup.readiness_timed_out")
        );
        assert_eq!(
            runtime_attention(
                &RuntimeError::InvalidResponse,
                PersistedSetupState::RunningTestInference,
            ),
            ("readiness_failed", "setup.readiness_failed")
        );
    }

    #[test]
    fn stale_catalogue_or_rule_versions_invalidate_an_approved_plan() {
        let capability = CapabilityEngine::v0_1();
        let (catalogue_version, rule_set_version) = capability.setup_versions();
        assert!(
            capability
                .validate_setup_versions(catalogue_version, rule_set_version)
                .is_ok()
        );
        assert_eq!(
            capability.validate_setup_versions(
                &CatalogueVersion::new("stale-catalogue"),
                rule_set_version,
            ),
            Err(CoreError::InvalidSetupPlan)
        );
        assert_eq!(
            capability
                .validate_setup_versions(catalogue_version, &RuleSetVersion::new("stale-rules"),),
            Err(CoreError::InvalidSetupPlan)
        );
    }

    #[tokio::test]
    async fn runtime_approval_evidence_must_still_match_the_persisted_plan() {
        let provider = Arc::new(fake_provider(Vec::new()));
        let (_temporary, _persistence, service) = service(provider);
        approve_runtime_reuse(&service).await;
        let health = service
            .runtime
            .status(
                service.provider.provider_id(),
                runtime_context(
                    CorrelationId::new(),
                    RequestId::new(),
                    RuntimeCancellationToken::new(),
                    Duration::from_secs(1),
                ),
            )
            .await
            .expect("approved fake runtime health loads");
        let mut job = PersistedSetupJob {
            plan: persisted_plan(SetupJobId::new()),
            state: PersistedSetupState::Approved,
            stage: PersistedSetupStage::Approval,
            progress: PersistedSetupProgress::default(),
            cancellation_requested: false,
            retry_count: 0,
            last_safe_error_code: None,
            attention_reason: None,
            runtime_verified_at_unix_ms: None,
            model_available_at_unix_ms: None,
            registration_verified_at_unix_ms: None,
            inference_verified_at_unix_ms: None,
            approved_at_unix_ms: Some(1),
            cancelled_at_unix_ms: None,
            last_event_sequence: 1,
            revision: 1,
            updated_at_unix_ms: 1,
        };

        assert!(ensure_runtime_ready(&health).is_ok());
        assert!(runtime_matches_plan(&health, &job));
        job.plan.external_runtime_effect = false;
        assert!(!runtime_matches_plan(&health, &job));
        job.plan.external_runtime_effect = true;
        job.plan.runtime_version = Some("stale-version".to_owned());
        assert!(!runtime_matches_plan(&health, &job));
    }

    struct SequencedStorageProvider {
        inner: FakeRuntimeProvider,
        preflights: Mutex<VecDeque<RuntimeStoragePreflight>>,
        fallback: RuntimeStoragePreflight,
        inspections: Mutex<VecDeque<RuntimeModelInspection>>,
        storage_requests: Mutex<Vec<u64>>,
    }

    impl RuntimeDetector for SequencedStorageProvider {
        fn detect(
            &self,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeObservation> {
            self.inner.detect(context)
        }
    }

    impl RuntimeLifecycle for SequencedStorageProvider {
        fn execute(
            &self,
            kind: RuntimeOperationKind,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeObservation> {
            self.inner.execute(kind, context)
        }
    }

    impl RuntimeModelInventoryProvider for SequencedStorageProvider {
        fn list_models(
            &self,
            limit: u16,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeModelInventory> {
            self.inner.list_models(limit, context)
        }
    }

    impl RuntimeModelSetupProvider for SequencedStorageProvider {
        fn prepare_model_acquisition(
            &self,
            model_id: CandidateModelId,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeModelAcquisitionPlan> {
            self.inner.prepare_model_acquisition(model_id, context)
        }

        fn preflight_model_storage(
            &self,
            _plan: RuntimeModelAcquisitionPlan,
            required_bytes: u64,
            safety_margin_bytes: u64,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeStoragePreflight> {
            Box::pin(async move {
                context.check()?;
                self.storage_requests
                    .lock()
                    .map_err(|_| RuntimeError::Internal)?
                    .push(required_bytes);
                let mut result = self
                    .preflights
                    .lock()
                    .map_err(|_| RuntimeError::Internal)?
                    .pop_front()
                    .unwrap_or_else(|| self.fallback.clone());
                result.required_bytes = required_bytes;
                result.safety_margin_bytes = safety_margin_bytes;
                Ok(result)
            })
        }

        fn acquire_model(
            &self,
            plan: RuntimeModelAcquisitionPlan,
            progress: ModelProgressSender,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeModelAcquisitionResult> {
            self.inner.acquire_model(plan, progress, context)
        }

        fn inspect_model(
            &self,
            artifact: ModelProviderArtifact,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeModelInspection> {
            if let Ok(mut inspections) = self.inspections.lock()
                && let Some(inspection) = inspections.pop_front()
            {
                return Box::pin(async move {
                    context.check()?;
                    Ok(inspection)
                });
            }
            self.inner.inspect_model(artifact, context)
        }

        fn run_readiness_inference(
            &self,
            artifact: ModelProviderArtifact,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeReadinessInferenceResult> {
            self.inner.run_readiness_inference(artifact, context)
        }
    }

    impl RuntimeChatProvider for SequencedStorageProvider {
        fn generate(
            &self,
            _request: RuntimeChatRequest,
            _deltas: ChatDeltaSender,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeGenerationResult> {
            // This double covers setup behavior only; chat is exercised elsewhere.
            Box::pin(async move {
                context.check()?;
                Err(RuntimeError::Unsupported)
            })
        }
    }

    impl RuntimeProvider for SequencedStorageProvider {
        fn provider_id(&self) -> &RuntimeProviderId {
            self.inner.provider_id()
        }
    }

    struct PendingAcquisitionProvider {
        inner: SequencedStorageProvider,
        started: Arc<tokio::sync::Notify>,
        stopped: Arc<AtomicBool>,
        progress: ModelAcquisitionProgress,
    }

    struct AcquisitionDropSignal(Arc<AtomicBool>);

    impl Drop for AcquisitionDropSignal {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    impl RuntimeDetector for PendingAcquisitionProvider {
        fn detect(
            &self,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeObservation> {
            self.inner.detect(context)
        }
    }

    impl RuntimeLifecycle for PendingAcquisitionProvider {
        fn execute(
            &self,
            kind: RuntimeOperationKind,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeObservation> {
            self.inner.execute(kind, context)
        }
    }

    impl RuntimeModelInventoryProvider for PendingAcquisitionProvider {
        fn list_models(
            &self,
            limit: u16,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeModelInventory> {
            self.inner.list_models(limit, context)
        }
    }

    impl RuntimeModelSetupProvider for PendingAcquisitionProvider {
        fn prepare_model_acquisition(
            &self,
            model_id: CandidateModelId,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeModelAcquisitionPlan> {
            self.inner.prepare_model_acquisition(model_id, context)
        }

        fn preflight_model_storage(
            &self,
            plan: RuntimeModelAcquisitionPlan,
            required_bytes: u64,
            safety_margin_bytes: u64,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeStoragePreflight> {
            self.inner
                .preflight_model_storage(plan, required_bytes, safety_margin_bytes, context)
        }

        fn acquire_model(
            &self,
            _plan: RuntimeModelAcquisitionPlan,
            progress: ModelProgressSender,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeModelAcquisitionResult> {
            let started = self.started.clone();
            let stopped = self.stopped.clone();
            let acquisition_progress = self.progress.clone();
            Box::pin(async move {
                context.check()?;
                let _drop_signal = AcquisitionDropSignal(stopped);
                progress
                    .send(acquisition_progress)
                    .await
                    .map_err(|_| RuntimeError::Internal)?;
                started.notify_waiters();
                std::future::pending::<()>().await;
                Err(RuntimeError::Internal)
            })
        }

        fn inspect_model(
            &self,
            artifact: ModelProviderArtifact,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeModelInspection> {
            self.inner.inspect_model(artifact, context)
        }

        fn run_readiness_inference(
            &self,
            artifact: ModelProviderArtifact,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeReadinessInferenceResult> {
            self.inner.run_readiness_inference(artifact, context)
        }
    }

    impl RuntimeChatProvider for PendingAcquisitionProvider {
        fn generate(
            &self,
            _request: RuntimeChatRequest,
            _deltas: ChatDeltaSender,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeGenerationResult> {
            // This double covers setup behavior only; chat is exercised elsewhere.
            Box::pin(async move {
                context.check()?;
                Err(RuntimeError::Unsupported)
            })
        }
    }

    impl RuntimeProvider for PendingAcquisitionProvider {
        fn provider_id(&self) -> &RuntimeProviderId {
            self.inner.provider_id()
        }
    }

    fn pending_provider() -> Arc<PendingAcquisitionProvider> {
        pending_provider_with_progress(ModelAcquisitionProgress {
            phase: ModelAcquisitionPhase::Transferring,
            completed_bytes: Some(256),
            total_bytes: Some(1_024),
            progress_basis_points: Some(2_500),
        })
    }

    fn pending_provider_with_progress(
        progress: ModelAcquisitionProgress,
    ) -> Arc<PendingAcquisitionProvider> {
        Arc::new(PendingAcquisitionProvider {
            inner: SequencedStorageProvider {
                inner: fake_provider(Vec::new()),
                preflights: Mutex::new(VecDeque::new()),
                fallback: available_storage(),
                inspections: Mutex::new(VecDeque::from([
                    unavailable_inspection(),
                    unavailable_inspection(),
                ])),
                storage_requests: Mutex::new(Vec::new()),
            },
            started: Arc::new(tokio::sync::Notify::new()),
            stopped: Arc::new(AtomicBool::new(false)),
            progress,
        })
    }

    struct PendingReadinessProvider {
        inner: Arc<dyn RuntimeProvider>,
        started: Arc<tokio::sync::Notify>,
        stopped: Arc<AtomicBool>,
    }

    impl RuntimeDetector for PendingReadinessProvider {
        fn detect(
            &self,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeObservation> {
            self.inner.detect(context)
        }
    }

    impl RuntimeLifecycle for PendingReadinessProvider {
        fn execute(
            &self,
            kind: RuntimeOperationKind,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeObservation> {
            self.inner.execute(kind, context)
        }
    }

    impl RuntimeModelInventoryProvider for PendingReadinessProvider {
        fn list_models(
            &self,
            limit: u16,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeModelInventory> {
            self.inner.list_models(limit, context)
        }
    }

    impl RuntimeModelSetupProvider for PendingReadinessProvider {
        fn prepare_model_acquisition(
            &self,
            model_id: CandidateModelId,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeModelAcquisitionPlan> {
            self.inner.prepare_model_acquisition(model_id, context)
        }

        fn preflight_model_storage(
            &self,
            plan: RuntimeModelAcquisitionPlan,
            required_bytes: u64,
            safety_margin_bytes: u64,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeStoragePreflight> {
            self.inner
                .preflight_model_storage(plan, required_bytes, safety_margin_bytes, context)
        }

        fn acquire_model(
            &self,
            plan: RuntimeModelAcquisitionPlan,
            progress: ModelProgressSender,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeModelAcquisitionResult> {
            self.inner.acquire_model(plan, progress, context)
        }

        fn inspect_model(
            &self,
            artifact: ModelProviderArtifact,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeModelInspection> {
            self.inner.inspect_model(artifact, context)
        }

        fn run_readiness_inference(
            &self,
            _artifact: ModelProviderArtifact,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeReadinessInferenceResult> {
            let started = self.started.clone();
            let stopped = self.stopped.clone();
            Box::pin(async move {
                context.check()?;
                let cancellation = context.cancellation();
                started.notify_waiters();
                cancellation.cancelled().await;
                stopped.store(true, Ordering::SeqCst);
                Err(RuntimeError::Cancelled)
            })
        }
    }

    impl RuntimeChatProvider for PendingReadinessProvider {
        fn generate(
            &self,
            _request: RuntimeChatRequest,
            _deltas: ChatDeltaSender,
            context: RuntimeOperationContext,
        ) -> RuntimeFuture<'_, RuntimeGenerationResult> {
            // This double covers setup behavior only; chat is exercised elsewhere.
            Box::pin(async move {
                context.check()?;
                Err(RuntimeError::Unsupported)
            })
        }
    }

    impl RuntimeProvider for PendingReadinessProvider {
        fn provider_id(&self) -> &RuntimeProviderId {
            self.inner.provider_id()
        }
    }

    #[tokio::test]
    async fn mid_acquisition_storage_loss_cancels_and_records_uncertain_retention() {
        let progress = ModelAcquisitionProgress {
            phase: ModelAcquisitionPhase::Transferring,
            completed_bytes: Some(512),
            total_bytes: Some(1_024),
            progress_basis_points: Some(STORAGE_CHECKPOINT_BASIS_POINTS),
        };
        let mut unavailable = available_storage();
        unavailable.availability = RuntimeStorageAvailability::Unavailable;
        unavailable.available_bytes = None;
        let provider = Arc::new(SequencedStorageProvider {
            inner: fake_provider(vec![progress]),
            preflights: Mutex::new(VecDeque::from([available_storage(), unavailable])),
            fallback: available_storage(),
            inspections: Mutex::new(VecDeque::from([unavailable_inspection()])),
            storage_requests: Mutex::new(Vec::new()),
        });
        let (_temporary, _persistence, service) = service(provider.clone());
        approve_runtime_reuse(&service).await;
        let job_id = persist_and_approve(&service).await;
        service
            .start_job(SetupJobStartRequest {
                job_id,
                plan_revision: 1,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("approved setup starts");

        let snapshot = wait_for_outcome(&service, job_id).await;
        assert_eq!(snapshot.state, SetupJobState::AttentionRequired);
        assert_eq!(
            snapshot.attention_reason,
            Some(SetupAttentionReason::DestinationUnavailable)
        );
        assert_eq!(
            snapshot.plan.model.lifecycle_state,
            ModelLifecycleState::AttentionRequired
        );
        assert!(snapshot.effect_report.as_ref().is_some_and(|report| {
            report.effects.iter().any(|effect| {
                effect.kind == SetupEffectKind::ProviderModelAcquisition
                    && effect.disposition == SetupEffectDisposition::Uncertain
            })
        }));
        assert_eq!(
            snapshot.error.as_ref().map(|error| error.code.as_str()),
            Some("setup.destination_unavailable")
        );
        assert_eq!(
            provider
                .storage_requests
                .lock()
                .expect("storage requests lock")
                .as_slice(),
            &[1_024, 512]
        );
    }

    #[tokio::test]
    async fn initial_insufficient_storage_never_starts_provider_acquisition() {
        let mut insufficient = available_storage();
        insufficient.availability = RuntimeStorageAvailability::InsufficientSpace;
        insufficient.available_bytes = Some(1_535);
        let provider = Arc::new(SequencedStorageProvider {
            inner: fake_provider(Vec::new()),
            preflights: Mutex::new(VecDeque::from([insufficient])),
            fallback: available_storage(),
            inspections: Mutex::new(VecDeque::from([unavailable_inspection()])),
            storage_requests: Mutex::new(Vec::new()),
        });
        let (_temporary, _persistence, service) = service(provider.clone());
        approve_runtime_reuse(&service).await;
        let job_id = persist_and_approve(&service).await;

        service
            .start_job(SetupJobStartRequest {
                job_id,
                plan_revision: 1,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("approved setup starts preflight");
        let snapshot = wait_for_outcome(&service, job_id).await;

        assert_eq!(snapshot.state, SetupJobState::AttentionRequired);
        assert_eq!(
            snapshot.attention_reason,
            Some(SetupAttentionReason::InsufficientStorage)
        );
        assert!(
            !provider
                .inner
                .calls()
                .contains(&FakeRuntimeCall::AcquireModel)
        );
        assert_eq!(
            snapshot.plan.model.lifecycle_state,
            ModelLifecycleState::Planned
        );
    }

    #[tokio::test]
    async fn registration_failure_never_marks_the_model_available() {
        let provider = Arc::new(SequencedStorageProvider {
            inner: fake_provider(Vec::new()),
            preflights: Mutex::new(VecDeque::new()),
            fallback: available_storage(),
            inspections: Mutex::new(VecDeque::from([
                unavailable_inspection(),
                model_inspection(
                    true,
                    ProviderRegistrationState::NotRegistered,
                    ModelIntegrityState::ProviderReported,
                ),
            ])),
            storage_requests: Mutex::new(Vec::new()),
        });
        let (_temporary, _persistence, service) = service(provider);
        approve_runtime_reuse(&service).await;
        let job_id = persist_and_approve(&service).await;

        service
            .start_job(SetupJobStartRequest {
                job_id,
                plan_revision: 1,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("approved setup starts");
        let snapshot = wait_for_outcome(&service, job_id).await;

        assert_eq!(snapshot.state, SetupJobState::AttentionRequired);
        assert_eq!(
            snapshot.attention_reason,
            Some(SetupAttentionReason::RegistrationUnverified)
        );
        assert_ne!(
            snapshot.plan.model.lifecycle_state,
            ModelLifecycleState::Available
        );
        assert_ne!(
            snapshot.plan.model.verification_state,
            ModelVerificationState::Verified
        );
    }

    #[tokio::test]
    async fn integrity_mismatch_never_runs_readiness_inference() {
        let provider = Arc::new(SequencedStorageProvider {
            inner: fake_provider(Vec::new()),
            preflights: Mutex::new(VecDeque::new()),
            fallback: available_storage(),
            inspections: Mutex::new(VecDeque::from([
                unavailable_inspection(),
                model_inspection(
                    true,
                    ProviderRegistrationState::Registered,
                    ModelIntegrityState::Mismatch,
                ),
            ])),
            storage_requests: Mutex::new(Vec::new()),
        });
        let (_temporary, _persistence, service) = service(provider.clone());
        approve_runtime_reuse(&service).await;
        let job_id = persist_and_approve(&service).await;

        service
            .start_job(SetupJobStartRequest {
                job_id,
                plan_revision: 1,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("approved setup starts");
        let snapshot = wait_for_outcome(&service, job_id).await;

        assert_eq!(snapshot.state, SetupJobState::Failed);
        assert_eq!(snapshot.stage, SetupStage::Failed);
        assert!(
            !provider
                .inner
                .calls()
                .contains(&FakeRuntimeCall::RunReadinessInference)
        );
        assert_eq!(
            snapshot.plan.model.lifecycle_state,
            ModelLifecycleState::Failed
        );
        assert_eq!(
            snapshot.plan.model.verification_state,
            ModelVerificationState::Failed
        );
        assert!(
            snapshot
                .plan
                .model
                .verification
                .as_ref()
                .is_some_and(|evidence| {
                    evidence.integrity == ModelIntegrityState::Mismatch
                        && !evidence.inference_verified
                })
        );
        assert_eq!(
            snapshot.error.as_ref().map(|error| error.code.as_str()),
            Some("setup.integrity_mismatch")
        );

        let status_request_id = RequestId::new();
        let reloaded = service
            .status(SetupJobStatusRequest {
                job_id,
                correlation_id: CorrelationId::new(),
                request_id: status_request_id,
            })
            .await
            .expect("failed snapshot reloads")
            .job;
        assert_eq!(
            reloaded.error.as_ref().map(|error| error.request_id),
            Some(status_request_id)
        );
        assert_eq!(
            service
                .retry_job(SetupJobRetryRequest {
                    job_id,
                    plan_revision: 1,
                    correlation_id: CorrelationId::new(),
                    request_id: RequestId::new(),
                })
                .await,
            Err(CoreError::InvalidSetupPlan)
        );
    }

    #[tokio::test]
    async fn readiness_timeout_preserves_only_unverified_retained_effects() {
        let provider = Arc::new(SequencedStorageProvider {
            inner: fake_provider_with_readiness(Vec::new(), Err(RuntimeError::TimedOut)),
            preflights: Mutex::new(VecDeque::new()),
            fallback: available_storage(),
            inspections: Mutex::new(VecDeque::from([unavailable_inspection()])),
            storage_requests: Mutex::new(Vec::new()),
        });
        let (_temporary, _persistence, service) = service(provider);
        approve_runtime_reuse(&service).await;
        let job_id = persist_and_approve(&service).await;

        service
            .start_job(SetupJobStartRequest {
                job_id,
                plan_revision: 1,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("approved setup starts");
        let snapshot = wait_for_outcome(&service, job_id).await;

        assert_eq!(snapshot.state, SetupJobState::AttentionRequired);
        assert_eq!(
            snapshot.attention_reason,
            Some(SetupAttentionReason::ReadinessTimedOut)
        );
        assert_ne!(
            snapshot.plan.model.lifecycle_state,
            ModelLifecycleState::Available
        );
        assert_ne!(
            snapshot.plan.model.verification_state,
            ModelVerificationState::Verified
        );
    }

    #[tokio::test]
    async fn readiness_inference_failure_never_marks_the_model_available() {
        let provider = Arc::new(SequencedStorageProvider {
            inner: fake_provider_with_readiness(
                Vec::new(),
                Err(RuntimeError::ReadinessInferenceFailed),
            ),
            preflights: Mutex::new(VecDeque::new()),
            fallback: available_storage(),
            inspections: Mutex::new(VecDeque::from([unavailable_inspection()])),
            storage_requests: Mutex::new(Vec::new()),
        });
        let (_temporary, _persistence, service) = service(provider);
        approve_runtime_reuse(&service).await;
        let job_id = persist_and_approve(&service).await;

        service
            .start_job(SetupJobStartRequest {
                job_id,
                plan_revision: 1,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("approved setup starts");
        let snapshot = wait_for_outcome(&service, job_id).await;

        assert_eq!(snapshot.state, SetupJobState::AttentionRequired);
        assert_eq!(
            snapshot.attention_reason,
            Some(SetupAttentionReason::ReadinessFailed)
        );
        assert_ne!(
            snapshot.plan.model.lifecycle_state,
            ModelLifecycleState::Available
        );
        assert_ne!(
            snapshot.plan.model.verification_state,
            ModelVerificationState::Verified
        );
    }

    #[tokio::test]
    async fn cancellation_during_readiness_verification_never_commits_ready() {
        let inner: Arc<dyn RuntimeProvider> = Arc::new(SequencedStorageProvider {
            inner: fake_provider(Vec::new()),
            preflights: Mutex::new(VecDeque::new()),
            fallback: available_storage(),
            inspections: Mutex::new(VecDeque::from([unavailable_inspection()])),
            storage_requests: Mutex::new(Vec::new()),
        });
        let provider = Arc::new(PendingReadinessProvider {
            inner,
            started: Arc::new(tokio::sync::Notify::new()),
            stopped: Arc::new(AtomicBool::new(false)),
        });
        let started = provider.started.notified();
        tokio::pin!(started);
        let (_temporary, _persistence, service) = service(provider.clone());
        approve_runtime_reuse(&service).await;
        let job_id = persist_and_approve(&service).await;

        service
            .start_job(SetupJobStartRequest {
                job_id,
                plan_revision: 1,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("approved setup starts");
        tokio::time::timeout(Duration::from_secs(1), &mut started)
            .await
            .expect("readiness verification starts");
        service
            .cancel_job(SetupJobCancelRequest {
                job_id,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("verification cancellation persists");
        let snapshot = wait_for_outcome(&service, job_id).await;
        tokio::time::timeout(Duration::from_secs(1), async {
            while !provider.stopped.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("provider observes verification cancellation");

        assert_eq!(snapshot.state, SetupJobState::Cancelled);
        assert_ne!(
            snapshot.plan.model.lifecycle_state,
            ModelLifecycleState::Available
        );
        assert_ne!(
            snapshot.plan.model.verification_state,
            ModelVerificationState::Verified
        );
        assert!(snapshot.cancellation_report.is_some());
        let effects = &snapshot
            .effect_report
            .as_ref()
            .expect("late cancellation preserves effect checkpoints")
            .effects;
        assert_eq!(
            effects
                .iter()
                .filter(|effect| effect.kind == SetupEffectKind::ProviderModelAcquisition)
                .count(),
            1
        );
        assert!(effects.iter().any(|effect| {
            effect.kind == SetupEffectKind::ProviderModelAcquisition
                && effect.disposition == SetupEffectDisposition::Completed
        }));
        assert!(effects.iter().any(|effect| {
            effect.kind == SetupEffectKind::ProviderModelRegistration
                && effect.disposition == SetupEffectDisposition::Completed
        }));
        assert!(effects.iter().any(|effect| {
            effect.kind == SetupEffectKind::ReadinessInference
                && effect.disposition == SetupEffectDisposition::Uncertain
        }));
    }

    #[tokio::test]
    async fn startup_recovery_marks_interrupted_acquisition_as_uncertain() {
        let provider = Arc::new(fake_provider(Vec::new()));
        let (_temporary, _persistence, service) = service(provider);
        let job_id = persist_and_approve(&service).await;
        for next in [
            PersistedSetupState::Preparing,
            PersistedSetupState::CheckingStorage,
            PersistedSetupState::Acquiring,
        ] {
            let current = service.load_job(job_id).await.expect("job reloads");
            service
                .persist_transition(
                    basic_transition(&current, next, CorrelationId::new())
                        .expect("recovery fixture transition is valid"),
                )
                .await
                .expect("interrupted stage persists");
        }

        let recovered = service
            .recover_job(SetupJobRecoveryRequest {
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("startup recovery succeeds")
            .job
            .expect("interrupted job is recovered");

        assert_eq!(recovered.state, SetupJobState::AttentionRequired);
        assert_eq!(
            recovered.attention_reason,
            Some(SetupAttentionReason::RecoveryRequired)
        );
        assert!(recovered.effect_report.as_ref().is_some_and(|report| {
            report.effects.iter().any(|effect| {
                effect.kind == SetupEffectKind::ProviderModelAcquisition
                    && effect.disposition == SetupEffectDisposition::Uncertain
            })
        }));
        assert_ne!(
            recovered.plan.model.lifecycle_state,
            ModelLifecycleState::Available
        );
    }

    #[tokio::test]
    async fn nonrecoverable_provider_contract_failure_is_terminal_failed() {
        let provider = Arc::new(fake_provider(Vec::new()));
        let (_temporary, _persistence, service) = service(provider);
        approve_runtime_reuse(&service).await;
        let job_id = persist_and_approve(&service).await;
        let approved = service.load_job(job_id).await.expect("approved job loads");
        service
            .persist_transition(
                basic_transition(
                    &approved,
                    PersistedSetupState::Preparing,
                    CorrelationId::new(),
                )
                .expect("preparation transition is valid"),
            )
            .await
            .expect("preparation persists");
        service
            .record_runtime_outcome(
                job_id,
                CorrelationId::new(),
                RuntimeError::Unsupported,
                None,
            )
            .await
            .expect("nonrecoverable provider contract failure persists");
        let snapshot = service
            .status(SetupJobStatusRequest {
                job_id,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("failed job status loads")
            .job;

        assert_eq!(snapshot.state, SetupJobState::Failed);
        assert_eq!(snapshot.stage, SetupStage::Failed);
        assert_eq!(
            snapshot.plan.model.lifecycle_state,
            ModelLifecycleState::Failed
        );
        assert_eq!(
            snapshot.plan.model.verification_state,
            ModelVerificationState::Failed
        );
        assert_eq!(
            snapshot.error.as_ref().map(|error| error.code.as_str()),
            Some("setup.provider_contract_unsupported")
        );
        assert_eq!(
            service
                .retry_job(SetupJobRetryRequest {
                    job_id,
                    plan_revision: 1,
                    correlation_id: CorrelationId::new(),
                    request_id: RequestId::new(),
                })
                .await,
            Err(CoreError::InvalidSetupPlan)
        );
    }

    #[tokio::test]
    async fn provider_neutral_integrity_error_persists_structured_mismatch_failure() {
        let provider = Arc::new(fake_provider(Vec::new()));
        let (_temporary, _persistence, service) = service(provider);
        approve_runtime_reuse(&service).await;
        let job_id = persist_and_approve(&service).await;
        let approved = service.load_job(job_id).await.expect("approved job loads");
        service
            .persist_transition(
                basic_transition(
                    &approved,
                    PersistedSetupState::Preparing,
                    CorrelationId::new(),
                )
                .expect("preparation transition is valid"),
            )
            .await
            .expect("preparation persists");

        service
            .record_runtime_outcome(
                job_id,
                CorrelationId::new(),
                RuntimeError::ModelIntegrityFailed,
                None,
            )
            .await
            .expect("integrity failure persists");
        let snapshot = service
            .status(SetupJobStatusRequest {
                job_id,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("integrity failure status loads")
            .job;

        assert_eq!(snapshot.state, SetupJobState::Failed);
        assert_eq!(snapshot.stage, SetupStage::Failed);
        assert_eq!(
            snapshot.plan.model.lifecycle_state,
            ModelLifecycleState::Failed
        );
        assert_eq!(
            snapshot.plan.model.verification_state,
            ModelVerificationState::Failed
        );
        assert!(
            snapshot
                .plan
                .model
                .verification
                .as_ref()
                .is_some_and(|evidence| evidence.integrity == ModelIntegrityState::Mismatch)
        );
        assert_eq!(
            snapshot.error.as_ref().map(|error| error.code.as_str()),
            Some("setup.integrity_mismatch")
        );
    }

    #[tokio::test]
    async fn exact_compatible_model_reuse_needs_only_margin_and_never_pulls() {
        let provider = Arc::new(SequencedStorageProvider {
            inner: fake_provider(Vec::new()),
            preflights: Mutex::new(VecDeque::new()),
            fallback: available_storage(),
            inspections: Mutex::new(VecDeque::new()),
            storage_requests: Mutex::new(Vec::new()),
        });
        let (_temporary, _persistence, service) = service(provider.clone());
        approve_runtime_reuse(&service).await;
        let job_id = persist_and_approve(&service).await;

        service
            .start_job(SetupJobStartRequest {
                job_id,
                plan_revision: 1,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("approved setup starts");
        let snapshot = wait_for_outcome(&service, job_id).await;

        assert_eq!(snapshot.state, SetupJobState::Ready);
        assert!(
            !provider
                .inner
                .calls()
                .contains(&FakeRuntimeCall::AcquireModel)
        );
        assert_eq!(
            provider
                .storage_requests
                .lock()
                .expect("storage requests lock")
                .as_slice(),
            &[0, 0, 0]
        );
        assert!(snapshot.effect_report.as_ref().is_some_and(|report| {
            !report
                .effects
                .iter()
                .any(|effect| effect.kind == SetupEffectKind::ProviderModelAcquisition)
        }));
    }

    #[tokio::test]
    async fn progress_persistence_failure_stops_provider_and_records_uncertain_effect() {
        let provider = pending_provider();
        let started = provider.started.notified();
        tokio::pin!(started);
        let (_temporary, _persistence, service) = service(provider.clone());
        approve_runtime_reuse(&service).await;
        let job_id = persist_and_approve(&service).await;
        service
            .fail_next_progress_persistence
            .store(true, Ordering::SeqCst);

        service
            .start_job(SetupJobStartRequest {
                job_id,
                plan_revision: 1,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("approved setup starts");
        tokio::time::timeout(Duration::from_secs(1), &mut started)
            .await
            .expect("pending provider starts acquisition");
        let snapshot = wait_for_outcome(&service, job_id).await;

        assert_eq!(snapshot.state, SetupJobState::AttentionRequired);
        assert_eq!(
            snapshot.attention_reason,
            Some(SetupAttentionReason::AcquisitionInterrupted)
        );
        assert!(provider.stopped.load(Ordering::SeqCst));
        assert!(snapshot.effect_report.as_ref().is_some_and(|report| {
            report.effects.iter().any(|effect| {
                effect.kind == SetupEffectKind::ProviderModelAcquisition
                    && effect.disposition == SetupEffectDisposition::Uncertain
            })
        }));
    }

    #[tokio::test]
    async fn progress_beyond_the_approved_size_stops_provider_and_requires_attention() {
        let provider = pending_provider_with_progress(ModelAcquisitionProgress {
            phase: ModelAcquisitionPhase::Transferring,
            completed_bytes: Some(1_025),
            total_bytes: Some(1_025),
            progress_basis_points: Some(10_000),
        });
        let started = provider.started.notified();
        tokio::pin!(started);
        let (_temporary, _persistence, service) = service(provider.clone());
        approve_runtime_reuse(&service).await;
        let job_id = persist_and_approve(&service).await;

        service
            .start_job(SetupJobStartRequest {
                job_id,
                plan_revision: 1,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("approved setup starts");
        tokio::time::timeout(Duration::from_secs(1), &mut started)
            .await
            .expect("provider reports its first progress update");
        let snapshot = wait_for_outcome(&service, job_id).await;

        assert_eq!(snapshot.state, SetupJobState::AttentionRequired);
        assert_eq!(
            snapshot.attention_reason,
            Some(SetupAttentionReason::AcquisitionInterrupted)
        );
        assert!(provider.stopped.load(Ordering::SeqCst));
        assert_ne!(
            snapshot.plan.model.lifecycle_state,
            ModelLifecycleState::Available
        );
    }

    #[tokio::test]
    async fn concurrent_duplicate_start_cannot_lose_the_cancellation_token() {
        let provider = pending_provider();
        let started = provider.started.notified();
        tokio::pin!(started);
        let (_temporary, _persistence, service) = service(provider.clone());
        approve_runtime_reuse(&service).await;
        let job_id = persist_and_approve(&service).await;
        service
            .start_job(SetupJobStartRequest {
                job_id,
                plan_revision: 1,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("first setup attempt starts");
        tokio::time::timeout(Duration::from_secs(1), &mut started)
            .await
            .expect("pending provider starts acquisition");

        let duplicate_service = service.clone();
        let duplicate = tokio::spawn(async move {
            duplicate_service
                .start_job(SetupJobStartRequest {
                    job_id,
                    plan_revision: 1,
                    correlation_id: CorrelationId::new(),
                    request_id: RequestId::new(),
                })
                .await
        });
        let cancel_service = service.clone();
        let cancel = tokio::spawn(async move {
            cancel_service
                .cancel_job(SetupJobCancelRequest {
                    job_id,
                    correlation_id: CorrelationId::new(),
                    request_id: RequestId::new(),
                })
                .await
        });
        let (_duplicate_result, cancel_result) = tokio::join!(duplicate, cancel);
        cancel_result
            .expect("cancel task joins")
            .expect("cancel intent persists");
        let snapshot = wait_for_outcome(&service, job_id).await;

        tokio::time::timeout(Duration::from_secs(1), async {
            while !provider.stopped.load(Ordering::SeqCst)
                || service.is_active(job_id).unwrap_or(true)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("cancelled provider task and active admission are released");

        assert_eq!(snapshot.state, SetupJobState::Cancelled);
        assert!(snapshot.cancellation_report.is_some());
        assert!(provider.stopped.load(Ordering::SeqCst));
        assert!(!service.is_active(job_id).expect("active registry reads"));
    }

    #[tokio::test]
    async fn cancellation_waits_for_atomic_admission_and_cannot_orphan_an_approved_job() {
        let provider = pending_provider();
        let (_temporary, _persistence, service) = service(provider.clone());
        approve_runtime_reuse(&service).await;
        let job_id = persist_and_approve(&service).await;
        service.pause_after_admission.store(true, Ordering::SeqCst);
        let admitted = service.admission_reached.notified();
        tokio::pin!(admitted);
        let start_service = service.clone();
        let start = tokio::spawn(async move {
            start_service
                .start_job(SetupJobStartRequest {
                    job_id,
                    plan_revision: 1,
                    correlation_id: CorrelationId::new(),
                    request_id: RequestId::new(),
                })
                .await
        });
        tokio::time::timeout(Duration::from_secs(1), &mut admitted)
            .await
            .expect("start reserves the active admission");
        let cancel_service = service.clone();
        let cancel = tokio::spawn(async move {
            cancel_service
                .cancel_job(SetupJobCancelRequest {
                    job_id,
                    correlation_id: CorrelationId::new(),
                    request_id: RequestId::new(),
                })
                .await
        });
        tokio::task::yield_now().await;
        assert!(!cancel.is_finished());
        service.admission_release.notify_waiters();
        start
            .await
            .expect("start task joins")
            .expect("admitted start commits its first checkpoint");
        let cancelled = cancel
            .await
            .expect("cancel task joins")
            .expect("queued cancellation commits")
            .job;
        tokio::time::timeout(Duration::from_secs(1), async {
            while service.is_active(job_id).unwrap_or(true) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("cancelled admission is released");

        assert_eq!(cancelled.state, SetupJobState::Cancelled);
        assert!(cancelled.cancellation_requested);
        assert!(!service.is_active(job_id).expect("active registry reads"));
    }

    #[tokio::test]
    async fn retry_is_idempotent_and_second_cancellation_uses_latest_event() {
        let provider = pending_provider();
        let first_started = provider.started.notified();
        tokio::pin!(first_started);
        let (_temporary, _persistence, service) = service(provider.clone());
        approve_runtime_reuse(&service).await;
        let job_id = persist_and_approve(&service).await;
        service
            .start_job(SetupJobStartRequest {
                job_id,
                plan_revision: 1,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("first attempt starts");
        tokio::time::timeout(Duration::from_secs(1), &mut first_started)
            .await
            .expect("first acquisition starts");
        let first = service
            .cancel_job(SetupJobCancelRequest {
                job_id,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("first cancellation persists")
            .job;
        let first_requested_at = first
            .cancellation_report
            .expect("first cancellation report exists")
            .requested_at_unix_ms;
        tokio::time::timeout(Duration::from_secs(1), async {
            while service.is_active(job_id).unwrap_or(true) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("first attempt releases its admission");
        provider.stopped.store(false, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(2)).await;

        let second_started = provider.started.notified();
        tokio::pin!(second_started);
        let retry_a = service.clone();
        let retry_b = service.clone();
        let (first_retry, duplicate_retry) = tokio::join!(
            retry_a.retry_job(SetupJobRetryRequest {
                job_id,
                plan_revision: 1,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            }),
            retry_b.retry_job(SetupJobRetryRequest {
                job_id,
                plan_revision: 1,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
        );
        first_retry.expect("retry starts");
        duplicate_retry.expect("duplicate retry is idempotent");
        tokio::time::timeout(Duration::from_secs(1), &mut second_started)
            .await
            .expect("second acquisition starts");
        let second = service
            .cancel_job(SetupJobCancelRequest {
                job_id,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("second cancellation persists")
            .job;
        let second_report = second
            .cancellation_report
            .expect("second cancellation report exists");

        assert_eq!(second.state, SetupJobState::Cancelled);
        assert_eq!(second.retry_count, 1);
        assert!(second_report.requested_at_unix_ms > first_requested_at);
    }

    #[tokio::test]
    async fn retry_limit_fails_before_provider_work_and_preserves_effect_capacity() {
        let provider = Arc::new(SequencedStorageProvider {
            inner: fake_provider_with_readiness(
                Vec::new(),
                Err(RuntimeError::ReadinessInferenceFailed),
            ),
            preflights: Mutex::new(VecDeque::new()),
            fallback: available_storage(),
            inspections: Mutex::new(VecDeque::from([unavailable_inspection()])),
            storage_requests: Mutex::new(Vec::new()),
        });
        let (_temporary, _persistence, service) = service(provider.clone());
        approve_runtime_reuse(&service).await;
        let job_id = persist_and_approve(&service).await;
        service
            .start_job(SetupJobStartRequest {
                job_id,
                plan_revision: 1,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("initial setup attempt starts");
        assert_eq!(
            wait_for_outcome(&service, job_id).await.state,
            SetupJobState::AttentionRequired
        );
        wait_for_inactive(&service, job_id).await;

        for retry_count in 1..=MAX_SETUP_RETRIES {
            service
                .retry_job(SetupJobRetryRequest {
                    job_id,
                    plan_revision: 1,
                    correlation_id: CorrelationId::new(),
                    request_id: RequestId::new(),
                })
                .await
                .expect("bounded retry starts");
            let snapshot = wait_for_outcome(&service, job_id).await;
            assert_eq!(snapshot.state, SetupJobState::AttentionRequired);
            assert_eq!(snapshot.retry_count, retry_count);
            wait_for_inactive(&service, job_id).await;
        }

        let persisted_job_id = persistence_job_id(job_id).expect("job id is valid");
        let effects_before = service
            .repository
            .effects(&persisted_job_id)
            .expect("effect history loads");
        let provider_calls_before = provider.inner.calls().len();
        let exhausted = service
            .retry_job(SetupJobRetryRequest {
                job_id,
                plan_revision: 1,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("retry exhaustion commits a terminal checkpoint")
            .job;
        let effects_after = service
            .repository
            .effects(&persisted_job_id)
            .expect("effect history remains readable");

        assert_eq!(exhausted.state, SetupJobState::Failed);
        assert_eq!(exhausted.retry_count, MAX_SETUP_RETRIES);
        assert_eq!(
            exhausted.error.as_ref().map(|error| error.code.as_str()),
            Some("setup.retry_limit_reached")
        );
        assert_eq!(
            exhausted.error.as_ref().map(|error| error.recovery.action),
            Some(RecoveryAction::ContactSupport)
        );
        assert_eq!(provider.inner.calls().len(), provider_calls_before);
        assert_eq!(effects_after, effects_before);
        assert!(effects_after.len() < 64);
        assert_eq!(
            service
                .retry_job(SetupJobRetryRequest {
                    job_id,
                    plan_revision: 1,
                    correlation_id: CorrelationId::new(),
                    request_id: RequestId::new(),
                })
                .await,
            Err(CoreError::InvalidSetupPlan)
        );

        let fresh_job_id = SetupJobId::new();
        let fresh = service
            .repository
            .create(&persisted_plan(fresh_job_id), CorrelationId::new())
            .expect("terminal exhaustion no longer blocks a fresh plan");
        assert_eq!(fresh.state, PersistedSetupState::AwaitingApproval);
    }

    #[tokio::test]
    async fn unknown_progress_is_ignored_before_monotonic_updates() {
        let provider = Arc::new(SequencedStorageProvider {
            inner: fake_provider(vec![
                ModelAcquisitionProgress {
                    phase: ModelAcquisitionPhase::Unknown,
                    completed_bytes: Some(50),
                    total_bytes: Some(100),
                    progress_basis_points: Some(5_000),
                },
                ModelAcquisitionProgress {
                    phase: ModelAcquisitionPhase::Transferring,
                    completed_bytes: Some(512),
                    total_bytes: Some(1_024),
                    progress_basis_points: Some(5_000),
                },
                ModelAcquisitionProgress {
                    phase: ModelAcquisitionPhase::Completed,
                    completed_bytes: Some(1_024),
                    total_bytes: Some(1_024),
                    progress_basis_points: Some(10_000),
                },
            ]),
            preflights: Mutex::new(VecDeque::new()),
            fallback: available_storage(),
            inspections: Mutex::new(VecDeque::from([unavailable_inspection()])),
            storage_requests: Mutex::new(Vec::new()),
        });
        let (_temporary, _persistence, service) = service(provider);
        approve_runtime_reuse(&service).await;
        let job_id = persist_and_approve(&service).await;

        service
            .start_job(SetupJobStartRequest {
                job_id,
                plan_revision: 1,
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .expect("approved setup starts");
        let snapshot = wait_for_outcome(&service, job_id).await;

        assert_eq!(snapshot.state, SetupJobState::Ready);
        assert_eq!(
            snapshot.plan.model.verification_state,
            ModelVerificationState::Verified
        );
    }
}
