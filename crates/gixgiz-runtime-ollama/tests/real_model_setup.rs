use std::{env, time::Duration};

use gixgiz_contracts::{CandidateModelId, CorrelationId, RequestId, RuntimeVersionCompatibility};
use gixgiz_runtime::{
    MODEL_PROGRESS_CHANNEL_CAPACITY, RuntimeDetector, RuntimeModelSetupProvider,
    RuntimeOperationContext, RuntimeStorageAvailability,
};
use gixgiz_runtime_ollama::OllamaRuntimeProvider;
use tokio::sync::mpsc;

const REAL_SETUP_SMOKE_ENV: &str = "GIXGIZ_RUN_REAL_OLLAMA_SETUP_SMOKE";
const REAL_ACQUISITION_APPROVAL_ENV: &str = "GIXGIZ_REAL_OLLAMA_ACQUISITION_APPROVED";
const COMPACT_MODEL_ID: &str = "qwen2.5.0.5b-instruct";
const GIB: u64 = 1024 * 1024 * 1024;

fn context(timeout: Duration) -> RuntimeOperationContext {
    RuntimeOperationContext::new(CorrelationId::new(), RequestId::new(), timeout)
}

#[tokio::test]
#[ignore = "downloads and retains an allowlisted model on an explicitly prepared machine"]
async fn real_allowlisted_model_setup_reaches_bounded_readiness() {
    assert_eq!(
        env::var(REAL_SETUP_SMOKE_ENV).as_deref(),
        Ok("1"),
        "set {REAL_SETUP_SMOKE_ENV}=1 only on an explicitly prepared local smoke-test machine"
    );
    assert_eq!(
        env::var(REAL_ACQUISITION_APPROVAL_ENV).as_deref(),
        Ok(COMPACT_MODEL_ID),
        "set {REAL_ACQUISITION_APPROVAL_ENV}={COMPACT_MODEL_ID} to approve this exact retained model effect"
    );
    let provider = OllamaRuntimeProvider::for_current_user().expect("provider construction");
    let observation = provider
        .detect(context(Duration::from_secs(10)))
        .await
        .expect("runtime detection");
    let version = observation
        .version
        .as_ref()
        .expect("reachable runtime exposes version evidence");
    assert_ne!(
        version.compatibility,
        RuntimeVersionCompatibility::Incompatible,
        "real setup smoke requires a supported or explicitly untested provider version"
    );

    let plan = provider
        .prepare_model_acquisition(
            CandidateModelId::new(COMPACT_MODEL_ID),
            context(Duration::from_secs(10)),
        )
        .await
        .expect("allowlisted provider mapping");
    let preflight = provider
        .preflight_model_storage(plan.clone(), GIB, 2 * GIB, context(Duration::from_secs(10)))
        .await
        .expect("provider-managed storage preflight");
    assert_eq!(
        preflight.availability,
        RuntimeStorageAvailability::Available,
        "real setup smoke requires the full conservative storage margin"
    );

    let (progress, mut updates) = mpsc::channel(MODEL_PROGRESS_CHANNEL_CAPACITY);
    let progress_task = tokio::spawn(async move {
        let mut update_count = 0_u64;
        while updates.recv().await.is_some() {
            update_count = update_count.saturating_add(1);
        }
        update_count
    });
    let acquisition = provider
        .acquire_model(
            plan.clone(),
            progress,
            context(Duration::from_secs(6 * 60 * 60)),
        )
        .await
        .expect("allowlisted provider acquisition");
    let update_count = progress_task.await.expect("progress collector");

    let inspection = provider
        .inspect_model(plan.artifact.clone(), context(Duration::from_secs(30)))
        .await
        .expect("exact provider registration inspection");
    assert!(
        inspection.available,
        "exact provider model must be available"
    );
    let readiness = provider
        .run_readiness_inference(plan.artifact, context(Duration::from_secs(180)))
        .await
        .expect("fixed bounded readiness inference");
    assert!(readiness.ready);

    eprintln!(
        "real setup smoke retained model; version={}, status={:?}, measured_bytes={:?}, normalized_updates={update_count}",
        version.reported_version, acquisition.status, acquisition.measured_size_bytes,
    );
}
