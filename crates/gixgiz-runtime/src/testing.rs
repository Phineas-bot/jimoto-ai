//! Deterministic fake runtime provider for core and host tests.

use std::sync::{Arc, Mutex};

use gixgiz_contracts::{
    CandidateModelId, ModelAcquisitionProgress, ModelProviderArtifact, RuntimeModelInventory,
    RuntimeOperationKind, RuntimeProviderId,
};

use crate::{
    ChatDeltaSender, ModelProgressSender, RuntimeChatProvider, RuntimeChatRequest, RuntimeDetector,
    RuntimeError, RuntimeFuture, RuntimeGenerationDelta, RuntimeGenerationResult, RuntimeLifecycle,
    RuntimeModelAcquisitionPlan, RuntimeModelAcquisitionResult, RuntimeModelInspection,
    RuntimeModelInventoryProvider, RuntimeModelSetupProvider, RuntimeObservation,
    RuntimeOperationContext, RuntimeProvider, RuntimeReadinessInferenceResult,
    RuntimeStoragePreflight,
};

/// Recorded deterministic fake-provider call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FakeRuntimeCall {
    /// Detection was requested.
    Detect,
    /// A lifecycle operation was requested.
    Lifecycle(RuntimeOperationKind),
    /// A model inventory was requested.
    ListModels(u16),
    /// Canonical model preparation was requested.
    PrepareModelAcquisition,
    /// Provider-managed storage preflight was requested.
    PreflightModelStorage,
    /// Exact provider artifact acquisition was requested.
    AcquireModel,
    /// Exact provider artifact inspection was requested.
    InspectModel,
    /// Fixed bounded readiness inference was requested.
    RunReadinessInference,
    /// A bounded streaming chat generation was requested.
    Generate,
}

/// Configurable provider that performs no process, filesystem, or network I/O.
#[derive(Clone)]
pub struct FakeRuntimeProvider {
    provider_id: RuntimeProviderId,
    observation: Arc<Mutex<Result<RuntimeObservation, RuntimeError>>>,
    lifecycle: Arc<Mutex<Result<RuntimeObservation, RuntimeError>>>,
    inventory: Arc<Mutex<Result<RuntimeModelInventory, RuntimeError>>>,
    acquisition_plan: Arc<Mutex<Result<RuntimeModelAcquisitionPlan, RuntimeError>>>,
    storage_preflight: Arc<Mutex<Result<RuntimeStoragePreflight, RuntimeError>>>,
    acquisition: Arc<Mutex<Result<RuntimeModelAcquisitionResult, RuntimeError>>>,
    inspection: Arc<Mutex<Result<RuntimeModelInspection, RuntimeError>>>,
    readiness: Arc<Mutex<Result<RuntimeReadinessInferenceResult, RuntimeError>>>,
    progress: Arc<Mutex<Vec<ModelAcquisitionProgress>>>,
    chat_deltas: Arc<Mutex<Vec<String>>>,
    chat_result: Arc<Mutex<Result<RuntimeGenerationResult, RuntimeError>>>,
    calls: Arc<Mutex<Vec<FakeRuntimeCall>>>,
}

impl FakeRuntimeProvider {
    /// Creates a fake with fixed deterministic results.
    #[must_use]
    pub fn new(
        provider_id: RuntimeProviderId,
        observation: Result<RuntimeObservation, RuntimeError>,
        lifecycle: Result<RuntimeObservation, RuntimeError>,
        inventory: Result<RuntimeModelInventory, RuntimeError>,
    ) -> Self {
        Self {
            provider_id,
            observation: Arc::new(Mutex::new(observation)),
            lifecycle: Arc::new(Mutex::new(lifecycle)),
            inventory: Arc::new(Mutex::new(inventory)),
            acquisition_plan: Arc::new(Mutex::new(Err(RuntimeError::Unsupported))),
            storage_preflight: Arc::new(Mutex::new(Err(RuntimeError::Unsupported))),
            acquisition: Arc::new(Mutex::new(Err(RuntimeError::Unsupported))),
            inspection: Arc::new(Mutex::new(Err(RuntimeError::Unsupported))),
            readiness: Arc::new(Mutex::new(Err(RuntimeError::Unsupported))),
            progress: Arc::new(Mutex::new(Vec::new())),
            chat_deltas: Arc::new(Mutex::new(Vec::new())),
            chat_result: Arc::new(Mutex::new(Err(RuntimeError::Unsupported))),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Configures the deltas and terminal outcome of one fake chat generation.
    ///
    /// Each delta is emitted only after a cancellation and deadline check, so
    /// tests can cancel deterministically between increments.
    #[must_use]
    pub fn with_chat_results(
        mut self,
        deltas: Vec<String>,
        result: Result<RuntimeGenerationResult, RuntimeError>,
    ) -> Self {
        self.chat_deltas = Arc::new(Mutex::new(deltas));
        self.chat_result = Arc::new(Mutex::new(result));
        self
    }

    /// Configures fixed deterministic model-setup results without changing the legacy constructor.
    #[must_use]
    pub fn with_model_setup_results(
        mut self,
        acquisition_plan: Result<RuntimeModelAcquisitionPlan, RuntimeError>,
        storage_preflight: Result<RuntimeStoragePreflight, RuntimeError>,
        acquisition: Result<RuntimeModelAcquisitionResult, RuntimeError>,
        inspection: Result<RuntimeModelInspection, RuntimeError>,
        readiness: Result<RuntimeReadinessInferenceResult, RuntimeError>,
    ) -> Self {
        self.acquisition_plan = Arc::new(Mutex::new(acquisition_plan));
        self.storage_preflight = Arc::new(Mutex::new(storage_preflight));
        self.acquisition = Arc::new(Mutex::new(acquisition));
        self.inspection = Arc::new(Mutex::new(inspection));
        self.readiness = Arc::new(Mutex::new(readiness));
        self
    }

    /// Configures normalized progress emitted during fake model acquisition.
    #[must_use]
    pub fn with_model_progress(mut self, progress: Vec<ModelAcquisitionProgress>) -> Self {
        self.progress = Arc::new(Mutex::new(progress));
        self
    }

    /// Returns the ordered call log, or an empty list after lock poisoning.
    #[must_use]
    pub fn calls(&self) -> Vec<FakeRuntimeCall> {
        self.calls
            .lock()
            .map_or_else(|_| Vec::new(), |calls| calls.clone())
    }

    fn record(&self, call: FakeRuntimeCall) -> Result<(), RuntimeError> {
        self.calls
            .lock()
            .map_err(|_| RuntimeError::Internal)?
            .push(call);
        Ok(())
    }
}

impl RuntimeDetector for FakeRuntimeProvider {
    fn detect(&self, context: RuntimeOperationContext) -> RuntimeFuture<'_, RuntimeObservation> {
        Box::pin(async move {
            context.check()?;
            self.record(FakeRuntimeCall::Detect)?;
            self.observation
                .lock()
                .map_err(|_| RuntimeError::Internal)?
                .clone()
        })
    }
}

impl RuntimeLifecycle for FakeRuntimeProvider {
    fn execute(
        &self,
        kind: RuntimeOperationKind,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeObservation> {
        Box::pin(async move {
            context.check()?;
            self.record(FakeRuntimeCall::Lifecycle(kind))?;
            self.lifecycle
                .lock()
                .map_err(|_| RuntimeError::Internal)?
                .clone()
        })
    }
}

impl RuntimeModelInventoryProvider for FakeRuntimeProvider {
    fn list_models(
        &self,
        limit: u16,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeModelInventory> {
        Box::pin(async move {
            context.check()?;
            self.record(FakeRuntimeCall::ListModels(limit))?;
            self.inventory
                .lock()
                .map_err(|_| RuntimeError::Internal)?
                .clone()
        })
    }
}

impl RuntimeModelSetupProvider for FakeRuntimeProvider {
    fn prepare_model_acquisition(
        &self,
        _model_id: CandidateModelId,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeModelAcquisitionPlan> {
        Box::pin(async move {
            context.check()?;
            self.record(FakeRuntimeCall::PrepareModelAcquisition)?;
            self.acquisition_plan
                .lock()
                .map_err(|_| RuntimeError::Internal)?
                .clone()
        })
    }

    fn preflight_model_storage(
        &self,
        _plan: RuntimeModelAcquisitionPlan,
        _required_bytes: u64,
        _safety_margin_bytes: u64,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeStoragePreflight> {
        Box::pin(async move {
            context.check()?;
            self.record(FakeRuntimeCall::PreflightModelStorage)?;
            self.storage_preflight
                .lock()
                .map_err(|_| RuntimeError::Internal)?
                .clone()
        })
    }

    fn acquire_model(
        &self,
        _plan: RuntimeModelAcquisitionPlan,
        progress: ModelProgressSender,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeModelAcquisitionResult> {
        Box::pin(async move {
            context.check()?;
            self.record(FakeRuntimeCall::AcquireModel)?;
            let updates = self
                .progress
                .lock()
                .map_err(|_| RuntimeError::Internal)?
                .clone();
            for update in updates {
                let _ = progress.try_send(update);
            }
            self.acquisition
                .lock()
                .map_err(|_| RuntimeError::Internal)?
                .clone()
        })
    }

    fn inspect_model(
        &self,
        _artifact: ModelProviderArtifact,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeModelInspection> {
        Box::pin(async move {
            context.check()?;
            self.record(FakeRuntimeCall::InspectModel)?;
            self.inspection
                .lock()
                .map_err(|_| RuntimeError::Internal)?
                .clone()
        })
    }

    fn run_readiness_inference(
        &self,
        _artifact: ModelProviderArtifact,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeReadinessInferenceResult> {
        Box::pin(async move {
            context.check()?;
            self.record(FakeRuntimeCall::RunReadinessInference)?;
            self.readiness
                .lock()
                .map_err(|_| RuntimeError::Internal)?
                .clone()
        })
    }
}

impl RuntimeChatProvider for FakeRuntimeProvider {
    fn generate(
        &self,
        _request: RuntimeChatRequest,
        deltas: ChatDeltaSender,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeGenerationResult> {
        Box::pin(async move {
            context.check()?;
            self.record(FakeRuntimeCall::Generate)?;
            let configured = self
                .chat_deltas
                .lock()
                .map_err(|_| RuntimeError::Internal)?
                .clone();
            let mut emitted_bytes = 0;
            for text in configured {
                context.check()?;
                emitted_bytes += text.len();
                if deltas
                    .send(RuntimeGenerationDelta {
                        text,
                        emitted_bytes,
                    })
                    .await
                    .is_err()
                {
                    break;
                }
            }
            context.check()?;
            self.chat_result
                .lock()
                .map_err(|_| RuntimeError::Internal)?
                .clone()
        })
    }
}

impl RuntimeProvider for FakeRuntimeProvider {
    fn provider_id(&self) -> &RuntimeProviderId {
        &self.provider_id
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use gixgiz_contracts::{
        CorrelationId, RUNTIME_REPORT_SCHEMA_VERSION, RequestId, RuntimeDisplayName,
        RuntimeEndpointSafety, RuntimeState,
    };

    use super::*;

    fn observation(provider_id: RuntimeProviderId) -> RuntimeObservation {
        RuntimeObservation {
            provider_id,
            display_name: RuntimeDisplayName::new("Fake runtime"),
            state: RuntimeState::Ready,
            endpoint_safety: RuntimeEndpointSafety::LoopbackVerified,
            version: None,
            capabilities: Vec::new(),
            reasons: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn inventory(provider_id: RuntimeProviderId) -> RuntimeModelInventory {
        RuntimeModelInventory {
            schema_version: RUNTIME_REPORT_SCHEMA_VERSION,
            provider_id,
            models: Vec::new(),
            truncated: false,
            collected_at_unix_ms: 1,
        }
    }

    fn context() -> RuntimeOperationContext {
        RuntimeOperationContext::new(
            CorrelationId::new(),
            RequestId::new(),
            Duration::from_secs(1),
        )
    }

    #[tokio::test]
    async fn fake_lifecycle_returns_configured_success_and_records_intent() {
        let provider_id = RuntimeProviderId::new("gixgiz.runtime.fake.v1");
        let expected = observation(provider_id.clone());
        let provider = FakeRuntimeProvider::new(
            provider_id.clone(),
            Ok(expected.clone()),
            Ok(expected.clone()),
            Ok(inventory(provider_id)),
        );

        let actual = provider
            .execute(RuntimeOperationKind::Restart, context())
            .await
            .expect("configured lifecycle succeeds");

        assert_eq!(actual, expected);
        assert_eq!(
            provider.calls(),
            vec![FakeRuntimeCall::Lifecycle(RuntimeOperationKind::Restart)]
        );
    }

    #[tokio::test]
    async fn fake_lifecycle_returns_configured_failure_without_side_effects() {
        let provider_id = RuntimeProviderId::new("gixgiz.runtime.fake.v1");
        let observed = observation(provider_id.clone());
        let provider = FakeRuntimeProvider::new(
            provider_id.clone(),
            Ok(observed),
            Err(RuntimeError::ProviderUnavailable),
            Ok(inventory(provider_id)),
        );

        let result = provider
            .execute(RuntimeOperationKind::Start, context())
            .await;

        assert_eq!(result, Err(RuntimeError::ProviderUnavailable));
        assert_eq!(
            provider.calls(),
            vec![FakeRuntimeCall::Lifecycle(RuntimeOperationKind::Start)]
        );
    }
}
