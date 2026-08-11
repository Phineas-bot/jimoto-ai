use std::{env, time::Duration};

use gixgiz_contracts::{
    CorrelationId, RequestId, RuntimeEndpointSafety, RuntimeState, RuntimeVersionCompatibility,
};
use gixgiz_runtime::{
    RuntimeDetector, RuntimeError, RuntimeModelInventoryProvider, RuntimeOperationContext,
};
use gixgiz_runtime_ollama::{OLLAMA_PROVIDER_ID, OllamaRuntimeProvider};

const REAL_SMOKE_ENV: &str = "GIXGIZ_RUN_REAL_OLLAMA_SMOKE";

fn context() -> RuntimeOperationContext {
    RuntimeOperationContext::new(
        CorrelationId::new(),
        RequestId::new(),
        Duration::from_secs(10),
    )
}

#[tokio::test]
#[ignore = "requires an explicitly prepared, running local Ollama instance"]
async fn real_local_provider_supports_read_only_status_and_inventory() {
    assert_eq!(
        env::var(REAL_SMOKE_ENV).as_deref(),
        Ok("1"),
        "set {REAL_SMOKE_ENV}=1 only on an explicitly prepared local smoke-test machine"
    );
    let provider = OllamaRuntimeProvider::for_current_user().expect("provider construction");

    let observation = provider.detect(context()).await.expect("runtime detection");
    assert_eq!(observation.provider_id.as_str(), OLLAMA_PROVIDER_ID);
    assert_eq!(
        observation.endpoint_safety,
        RuntimeEndpointSafety::LoopbackVerified
    );
    let version = observation
        .version
        .as_ref()
        .expect("reachable observation records a provider version");
    let expected_state = match version.compatibility {
        RuntimeVersionCompatibility::Compatible => RuntimeState::Ready,
        RuntimeVersionCompatibility::Incompatible => RuntimeState::Incompatible,
        RuntimeVersionCompatibility::Untested => RuntimeState::Degraded,
        _ => RuntimeState::Degraded,
    };
    assert_eq!(observation.state, expected_state);
    eprintln!(
        "real Ollama smoke provider version: {}",
        version.reported_version
    );

    let inventory = provider.list_models(16, context()).await;
    if version.compatibility == RuntimeVersionCompatibility::Incompatible {
        assert!(matches!(inventory, Err(RuntimeError::IncompatibleVersion)));
        return;
    }
    let inventory = inventory.expect("policy permits read-only model inventory");
    assert_eq!(inventory.provider_id.as_str(), OLLAMA_PROVIDER_ID);
    assert!(inventory.models.len() <= 16);
}
