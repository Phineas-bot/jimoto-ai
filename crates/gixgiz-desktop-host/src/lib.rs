//! Authenticated loopback composition boundary for the GixGiz desktop sidecar.
//!
//! This crate owns only process bootstrap, supervision, and transport
//! adaptation. Platform policy remains in `gixgiz-core`, while serialized
//! boundary types remain in `gixgiz-contracts`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod bootstrap;
mod error;
mod hardware_scans;
mod operations;
mod runtime_operations;
mod server;

use std::time::Duration;

use gixgiz_contracts::{
    BootstrapReady, CorrelationId, InstanceId, RequestId, SetupJobRecoveryRequest,
    StorageLocationKind,
};
use gixgiz_core::{CoreError, HardwareProvider, HardwareScanner, OperationContext, PlatformCore};
use gixgiz_runtime_ollama::{OLLAMA_PROVIDER_ID, OllamaInstaller, OllamaRuntimeProvider};
use tokio::io::{AsyncWriteExt, BufReader};

pub use error::HostError;

use crate::{bootstrap::read_bootstrap, server::SidecarHost};

const CORE_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);

/// Reads the inherited bootstrap pipe, runs the loopback host, and shuts the
/// core down within a fixed deadline.
pub async fn run_sidecar() -> Result<(), HostError> {
    let stdin = BufReader::new(tokio::io::stdin());
    let bootstrap = read_bootstrap(stdin).await?;
    let instance_id = InstanceId::new();
    let context = OperationContext::generated();

    let (
        mut core,
        status,
        runtime_service,
        setup_service,
        chat_service,
        install_coordinator,
        storage_locations,
    ) = tokio::task::spawn_blocking(move || {
        let mut core = PlatformCore::with_default_persistence();

        // Locations are read before the provider is composed, so a runtime
        // installed outside the default directory is still discovered.
        let chosen_runtime = core.storage_location(StorageLocationKind::Runtime);
        let chosen_staging = core.storage_location(StorageLocationKind::Staging);
        let runtime_provider = std::sync::Arc::new(OllamaRuntimeProvider::from_environment(
            chosen_runtime
                .as_ref()
                .map(|root| root.join("Ollama").join("ollama.exe")),
        ));

        let runtime_service = core.runtime_service(runtime_provider.clone());
        let setup_service = core.setup_service(runtime_provider.clone());
        let chat_service = core.chat_service(runtime_provider.clone());
        let status = core.start(&context)?;
        // Managed installation stages into the GixGiz-owned staging root and
        // reuses ordinary detection for post-install verification.
        let install_coordinator = chosen_staging
            .or_else(|| core.data_root_staging())
            .and_then(|staging| {
                let installer = std::sync::Arc::new(OllamaInstaller::for_current_user(
                    staging,
                    chosen_runtime.clone(),
                    runtime_provider.clone(),
                ));
                core.runtime_install_coordinator(installer, OLLAMA_PROVIDER_ID)
            });
        let storage_locations = core.storage_location_service();
        Ok::<_, gixgiz_core::CoreError>((
            core,
            status,
            runtime_service,
            setup_service,
            chat_service,
            install_coordinator,
            storage_locations,
        ))
    })
    .await
    .map_err(HostError::CoreWorker)?
    .map_err(HostError::Core)?;
    let hardware_scanner = HardwareScanner::windows().unwrap_or_else(|error| {
        HardwareScanner::new(std::sync::Arc::new(UnavailableHardwareProvider(error)))
    });
    if let Some(service) = setup_service.as_ref() {
        service
            .recover_job(SetupJobRecoveryRequest {
                correlation_id: CorrelationId::new(),
                request_id: RequestId::new(),
            })
            .await
            .map_err(HostError::Core)?;
    }

    if let Some(service) = chat_service.as_ref() {
        // A reply interrupted by an earlier exit can never resume as completed.
        service
            .recover_interrupted()
            .await
            .map_err(HostError::Core)?;
    }

    if let Some(coordinator) = install_coordinator.as_ref() {
        // An installation interrupted by an earlier exit is reconciled through
        // fresh detection before any client can observe it.
        coordinator
            .recover(gixgiz_runtime::RuntimeOperationContext::new(
                CorrelationId::new(),
                RequestId::new(),
                Duration::from_secs(60),
            ))
            .await
            .map_err(HostError::Core)?;
    }

    let serve_result = async {
        let host = SidecarHost::bind(
            bootstrap.token.clone(),
            instance_id,
            status,
            hardware_scanner,
            crate::server::SidecarServices {
                runtime: runtime_service,
                setup: setup_service,
                chat: chat_service,
                runtime_install: install_coordinator,
                storage_locations,
            },
        )
        .await?;
        let ready = BootstrapReady::new(
            host.local_addr().port(),
            instance_id,
            gixgiz_contracts::PROTOCOL_VERSION,
            gixgiz_contracts::PROTOCOL_VERSION,
        );
        write_bootstrap_ready(&ready).await?;

        let shutdown = host.shutdown_handle();
        tokio::spawn(async move {
            bootstrap.monitor_supervisor().await;
            shutdown.request();
        });

        host.run().await
    }
    .await;
    let shutdown_context = OperationContext::generated().with_timeout(CORE_SHUTDOWN_TIMEOUT);
    let core_result = tokio::task::spawn_blocking(move || core.shutdown(&shutdown_context))
        .await
        .map_err(HostError::CoreWorker)
        .and_then(|result| result.map_err(HostError::Core));

    serve_result.and(core_result)
}

struct UnavailableHardwareProvider(CoreError);

impl HardwareProvider for UnavailableHardwareProvider {
    fn collect(
        &self,
        _context: &OperationContext,
    ) -> Result<gixgiz_core::CollectedHardwareEvidence, CoreError> {
        Err(self.0.clone())
    }
}

async fn write_bootstrap_ready(ready: &BootstrapReady) -> Result<(), HostError> {
    let mut output = serde_json::to_vec(ready).map_err(|_| HostError::BootstrapResponse)?;
    output.push(b'\n');
    let mut stdout = tokio::io::stdout();
    stdout
        .write_all(&output)
        .await
        .map_err(|_| HostError::BootstrapResponse)?;
    stdout
        .flush()
        .await
        .map_err(|_| HostError::BootstrapResponse)
}
