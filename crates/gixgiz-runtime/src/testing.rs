//! Deterministic fake runtime provider for core and host tests.

use std::sync::{Arc, Mutex};

use gixgiz_contracts::{RuntimeModelInventory, RuntimeOperationKind, RuntimeProviderId};

use crate::{
    RuntimeDetector, RuntimeError, RuntimeFuture, RuntimeLifecycle, RuntimeModelInventoryProvider,
    RuntimeObservation, RuntimeOperationContext, RuntimeProvider,
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
}

/// Configurable provider that performs no process, filesystem, or network I/O.
#[derive(Clone)]
pub struct FakeRuntimeProvider {
    provider_id: RuntimeProviderId,
    observation: Arc<Mutex<Result<RuntimeObservation, RuntimeError>>>,
    lifecycle: Arc<Mutex<Result<RuntimeObservation, RuntimeError>>>,
    inventory: Arc<Mutex<Result<RuntimeModelInventory, RuntimeError>>>,
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
            calls: Arc::new(Mutex::new(Vec::new())),
        }
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
