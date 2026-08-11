use std::path::Path;

use gixgiz_contracts::{ServiceHealth, ServiceHealthStatus, ServiceRequirement};
use gixgiz_persistence::{DataRoot, Persistence, PersistenceError};

use crate::{CoreError, OperationContext, ServiceHealthSource};

const SERVICE_ID: &str = "persistence";

pub(crate) struct PersistenceHealthSource {
    state: PersistenceState,
}

enum PersistenceState {
    Open(Persistence),
    Failed {
        status: ServiceHealthStatus,
        message: &'static str,
    },
}

impl PersistenceHealthSource {
    pub(crate) fn open_default() -> (Self, Result<Persistence, ()>) {
        Self::from_result(DataRoot::resolve_default().and_then(Persistence::open))
    }

    pub(crate) fn open_override(root: impl AsRef<Path>) -> (Self, Result<Persistence, ()>) {
        Self::from_result(DataRoot::from_override(root).and_then(Persistence::open))
    }

    fn from_result(
        result: Result<Persistence, PersistenceError>,
    ) -> (Self, Result<Persistence, ()>) {
        let (state, access) = match result {
            Ok(persistence) => (PersistenceState::Open(persistence.clone()), Ok(persistence)),
            Err(error) => (
                PersistenceState::Failed {
                    status: error.health_status(),
                    message: error.safe_health_message(),
                },
                Err(()),
            ),
        };
        (Self { state }, access)
    }
}

impl ServiceHealthSource for PersistenceHealthSource {
    fn health(&self, _context: &OperationContext) -> Result<ServiceHealth, CoreError> {
        let (status, message) = match &self.state {
            PersistenceState::Open(persistence) => match persistence.health_check() {
                Ok(health) => (
                    ServiceHealthStatus::Healthy,
                    format!(
                        "Local persistence is healthy at schema version {}.",
                        health.schema_version
                    ),
                ),
                Err(error) => (
                    error.health_status(),
                    error.safe_health_message().to_owned(),
                ),
            },
            PersistenceState::Failed { status, message } => (*status, (*message).to_owned()),
        };
        Ok(ServiceHealth::new(
            SERVICE_ID,
            "Local persistence",
            ServiceRequirement::Mandatory,
            status,
            Some(message),
        ))
    }
}
