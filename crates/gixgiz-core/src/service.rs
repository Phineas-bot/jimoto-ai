use std::{fmt, path::Path};

use gixgiz_contracts::{
    ApplicationInfo, PlatformStatus, ReadinessReport, ReadinessStatus, ServiceHealth,
    ServiceHealthStatus, ServiceRequirement,
};

use gixgiz_persistence::Persistence;
use gixgiz_runtime::RuntimeProvider;

use crate::{
    ChatService, CoreError, OperationContext, RuntimeService, SetupService,
    persistence::PersistenceHealthSource,
};

/// Explicit in-process lifecycle of the Task 03 platform core.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreLifecycle {
    /// The service has been constructed but not started.
    Created,
    /// Startup health collection is in progress.
    Starting,
    /// The service is running and can report health.
    Running,
    /// Shutdown is in progress.
    Stopping,
    /// The service completed shutdown.
    Stopped,
    /// Startup health collection failed.
    Failed,
}

impl fmt::Display for CoreLifecycle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

/// Provider-neutral dependency that supplies one service health record.
pub trait ServiceHealthSource: Send + Sync {
    /// Collects one safe service-health record for the current operation.
    fn health(&self, context: &OperationContext) -> Result<ServiceHealth, CoreError>;
}

/// Minimal platform service that owns lifecycle and readiness composition.
pub struct PlatformCore {
    lifecycle: CoreLifecycle,
    health_sources: Vec<Box<dyn ServiceHealthSource>>,
    persistence: PersistenceAccess,
}

enum PersistenceAccess {
    NotConfigured,
    Available(Persistence),
    Unavailable,
}

impl PlatformCore {
    /// Creates a core with provider-neutral health dependencies.
    #[must_use]
    pub fn new(health_sources: Vec<Box<dyn ServiceHealthSource>>) -> Self {
        Self {
            lifecycle: CoreLifecycle::Created,
            health_sources,
            persistence: PersistenceAccess::NotConfigured,
        }
    }

    /// Creates a production core with the default per-user persistence root.
    ///
    /// This performs blocking filesystem and SQLite initialization. Async hosts
    /// must call it from a blocking worker.
    #[must_use]
    pub fn with_default_persistence() -> Self {
        let (source, persistence) = PersistenceHealthSource::open_default();
        Self {
            lifecycle: CoreLifecycle::Created,
            health_sources: vec![Box::new(source)],
            persistence: persistence
                .map(PersistenceAccess::Available)
                .unwrap_or(PersistenceAccess::Unavailable),
        }
    }

    /// Creates a core with persistence beneath an explicit controlled root.
    ///
    /// Tests should use a temporary directory and must not resolve the user's
    /// default application-data location.
    #[must_use]
    pub fn with_persistence_root(root: impl AsRef<Path>) -> Self {
        let (source, persistence) = PersistenceHealthSource::open_override(root);
        Self {
            lifecycle: CoreLifecycle::Created,
            health_sources: vec![Box::new(source)],
            persistence: persistence
                .map(PersistenceAccess::Available)
                .unwrap_or(PersistenceAccess::Unavailable),
        }
    }

    /// Composes provider-neutral runtime policy with the core's persistence owner.
    #[must_use]
    pub fn runtime_service(&self, provider: std::sync::Arc<dyn RuntimeProvider>) -> RuntimeService {
        match &self.persistence {
            PersistenceAccess::Available(persistence) => {
                RuntimeService::with_persistence(provider, persistence)
            }
            PersistenceAccess::NotConfigured => RuntimeService::in_memory(provider),
            PersistenceAccess::Unavailable => RuntimeService::policy_unavailable(provider),
        }
    }

    /// Composes durable model setup independently from the core lifecycle state.
    ///
    /// `None` means persistence failed to initialize or was not configured; setup
    /// never falls back to volatile state because approval and recovery are durable.
    #[must_use]
    pub fn setup_service(
        &self,
        provider: std::sync::Arc<dyn RuntimeProvider>,
    ) -> Option<SetupService> {
        match &self.persistence {
            PersistenceAccess::Available(persistence) => {
                Some(SetupService::with_persistence(provider, persistence))
            }
            PersistenceAccess::NotConfigured | PersistenceAccess::Unavailable => None,
        }
    }

    /// Composes local chat with the core's Rust-owned persistence.
    #[must_use]
    pub fn chat_service(
        &self,
        provider: std::sync::Arc<dyn RuntimeProvider>,
    ) -> Option<ChatService> {
        match &self.persistence {
            PersistenceAccess::Available(persistence) => {
                Some(ChatService::with_persistence(provider, persistence))
            }
            PersistenceAccess::NotConfigured | PersistenceAccess::Unavailable => None,
        }
    }

    /// Composes the storage-location service with Rust-owned persistence.
    #[must_use]
    pub fn storage_location_service(
        &self,
    ) -> Option<std::sync::Arc<crate::StorageLocationService>> {
        match &self.persistence {
            PersistenceAccess::Available(persistence) => Some(std::sync::Arc::new(
                crate::StorageLocationService::with_persistence(
                    persistence,
                    std::sync::Arc::new(crate::WindowsFreeSpaceProbe),
                ),
            )),
            PersistenceAccess::NotConfigured | PersistenceAccess::Unavailable => None,
        }
    }

    /// Returns a user-selected storage location, when one was chosen.
    ///
    /// Exposed on the core because persistence ownership stays here: the host
    /// composes services from these paths without opening the database itself.
    #[must_use]
    pub fn storage_location(
        &self,
        kind: gixgiz_contracts::StorageLocationKind,
    ) -> Option<std::path::PathBuf> {
        match &self.persistence {
            PersistenceAccess::Available(persistence) => {
                let service = crate::StorageLocationService::with_persistence(
                    persistence,
                    std::sync::Arc::new(crate::WindowsFreeSpaceProbe),
                );
                service.resolved(kind).ok().flatten()
            }
            PersistenceAccess::NotConfigured | PersistenceAccess::Unavailable => None,
        }
    }

    /// Returns the GixGiz-owned staging directory when persistence is available.
    ///
    /// Installer artifacts are staged here and nowhere else, so cleanup can only
    /// ever remove GixGiz-owned files.
    #[must_use]
    pub fn data_root_staging(&self) -> Option<std::path::PathBuf> {
        match &self.persistence {
            PersistenceAccess::Available(persistence) => {
                Some(persistence.data_root().staging_dir().to_path_buf())
            }
            PersistenceAccess::NotConfigured | PersistenceAccess::Unavailable => None,
        }
    }

    /// Composes managed runtime installation with Rust-owned persistence.
    ///
    /// Returns `None` when persistence is unavailable, so an approved system
    /// change can never run without a durable record of it.
    #[must_use]
    pub fn runtime_install_coordinator(
        &self,
        installer: std::sync::Arc<dyn gixgiz_runtime::RuntimeInstaller>,
        provider_id: &str,
    ) -> Option<std::sync::Arc<crate::RuntimeInstallCoordinator>> {
        match &self.persistence {
            PersistenceAccess::Available(persistence) => Some(std::sync::Arc::new(
                crate::RuntimeInstallCoordinator::with_persistence(
                    installer,
                    persistence,
                    provider_id,
                ),
            )),
            PersistenceAccess::NotConfigured | PersistenceAccess::Unavailable => None,
        }
    }

    /// Returns the current explicit lifecycle state.
    #[must_use]
    pub const fn lifecycle(&self) -> CoreLifecycle {
        self.lifecycle
    }

    /// Returns compiled application and protocol version information.
    #[must_use]
    pub fn version(&self) -> ApplicationInfo {
        ApplicationInfo::current()
    }

    /// Starts the core, verifies health sources, and returns initial readiness.
    pub fn start(&mut self, context: &OperationContext) -> Result<PlatformStatus, CoreError> {
        context.check()?;

        match self.lifecycle {
            CoreLifecycle::Running => return self.collect_status(context),
            CoreLifecycle::Created | CoreLifecycle::Stopped | CoreLifecycle::Failed => {}
            CoreLifecycle::Starting | CoreLifecycle::Stopping => {
                return Err(CoreError::LifecycleConflict {
                    operation: "start",
                    state: self.lifecycle,
                });
            }
        }

        self.lifecycle = CoreLifecycle::Starting;
        match self.collect_status(context) {
            Ok(status) => {
                self.lifecycle = CoreLifecycle::Running;
                tracing::info!(
                    correlation_id = %context.correlation_id(),
                    request_id = %context.request_id(),
                    lifecycle = "running",
                    "platform core started"
                );
                Ok(status)
            }
            Err(error) => {
                self.lifecycle = CoreLifecycle::Failed;
                Err(error)
            }
        }
    }

    /// Returns deterministic version and health information while running.
    pub fn health_and_version(
        &self,
        context: &OperationContext,
    ) -> Result<PlatformStatus, CoreError> {
        context.check()?;
        if self.lifecycle != CoreLifecycle::Running {
            return Err(CoreError::LifecycleConflict {
                operation: "health_and_version",
                state: self.lifecycle,
            });
        }

        self.collect_status(context)
    }

    /// Stops the core without performing external system work.
    pub fn shutdown(&mut self, context: &OperationContext) -> Result<(), CoreError> {
        context.check()?;

        match self.lifecycle {
            CoreLifecycle::Created => {
                self.lifecycle = CoreLifecycle::Stopped;
            }
            CoreLifecycle::Running | CoreLifecycle::Failed => {
                self.lifecycle = CoreLifecycle::Stopping;
                self.lifecycle = CoreLifecycle::Stopped;
                tracing::info!(
                    correlation_id = %context.correlation_id(),
                    request_id = %context.request_id(),
                    lifecycle = "stopped",
                    "platform core stopped"
                );
            }
            CoreLifecycle::Stopped => {}
            CoreLifecycle::Starting | CoreLifecycle::Stopping => {
                return Err(CoreError::LifecycleConflict {
                    operation: "shutdown",
                    state: self.lifecycle,
                });
            }
        }

        Ok(())
    }

    fn collect_status(&self, context: &OperationContext) -> Result<PlatformStatus, CoreError> {
        let mut services = vec![ServiceHealth::new(
            "platform_core",
            "Platform core",
            ServiceRequirement::Mandatory,
            ServiceHealthStatus::Healthy,
            Some("The platform core foundation is responsive.".to_owned()),
        )];
        let dependency_health = self
            .health_sources
            .iter()
            .map(|source| {
                context.check()?;
                source.health(context)
            })
            .collect::<Result<Vec<_>, _>>()?;
        services.extend(dependency_health);

        Ok(PlatformStatus {
            application: self.version(),
            readiness: compose_readiness(services),
        })
    }
}

/// Applies conservative, provider-neutral readiness precedence to service evidence.
#[must_use]
pub fn compose_readiness(services: Vec<ServiceHealth>) -> ReadinessReport {
    let has_mandatory_failure = services.iter().any(|service| {
        service.requirement.is_mandatory() && service.status == ServiceHealthStatus::Failed
    });
    let has_mandatory_unavailable = services.iter().any(|service| {
        service.requirement.is_mandatory()
            && matches!(
                service.status,
                ServiceHealthStatus::Unavailable | ServiceHealthStatus::Unknown
            )
    });
    let has_degraded_evidence = services
        .iter()
        .any(|service| service.status != ServiceHealthStatus::Healthy);

    let (status, summary) = if services.is_empty() {
        (
            ReadinessStatus::Unavailable,
            "No service health evidence is available.",
        )
    } else if has_mandatory_failure {
        (
            ReadinessStatus::Failed,
            "A mandatory platform service failed.",
        )
    } else if has_mandatory_unavailable {
        (
            ReadinessStatus::Unavailable,
            "A mandatory platform service is unavailable.",
        )
    } else if has_degraded_evidence {
        (
            ReadinessStatus::Degraded,
            "The platform core is available with reduced capability.",
        )
    } else {
        (
            ReadinessStatus::Ready,
            "All mandatory platform services are healthy.",
        )
    };

    ReadinessReport {
        status,
        summary: summary.to_owned(),
        services,
    }
}
