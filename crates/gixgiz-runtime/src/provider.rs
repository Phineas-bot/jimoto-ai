use std::{future::Future, pin::Pin};

use gixgiz_contracts::{
    RuntimeCapabilityDescriptor, RuntimeDisplayName, RuntimeEndpointSafety, RuntimeModelInventory,
    RuntimeOperationKind, RuntimeProviderId, RuntimeReason, RuntimeState, RuntimeVersionInfo,
    RuntimeWarning,
};

use crate::{RuntimeError, RuntimeModelSetupProvider, RuntimeOperationContext};

/// Sendable boxed future used by object-safe runtime provider traits.
pub type RuntimeFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, RuntimeError>> + Send + 'a>>;

/// Provider evidence before core ownership and consent policy are applied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeObservation {
    /// Stable provider identity.
    pub provider_id: RuntimeProviderId,
    /// Bounded provider display name.
    pub display_name: RuntimeDisplayName,
    /// Normalized provider state derived from current evidence.
    pub state: RuntimeState,
    /// Safety of the adapter-selected endpoint.
    pub endpoint_safety: RuntimeEndpointSafety,
    /// Normalized version evidence when available.
    pub version: Option<RuntimeVersionInfo>,
    /// Provider capability support before core consent policy.
    pub capabilities: Vec<RuntimeCapabilityDescriptor>,
    /// Safe reasons supporting the observation.
    pub reasons: Vec<RuntimeReason>,
    /// Safe warnings attached to incomplete evidence.
    pub warnings: Vec<RuntimeWarning>,
}

/// Detects installation, endpoint, version, and health evidence without mutation.
pub trait RuntimeDetector: Send + Sync {
    /// Returns current normalized provider evidence.
    fn detect(&self, context: RuntimeOperationContext) -> RuntimeFuture<'_, RuntimeObservation>;
}

/// Performs only bounded provider lifecycle actions supported by the adapter.
pub trait RuntimeLifecycle: Send + Sync {
    /// Executes one start, stop, or restart operation and re-observes status.
    fn execute(
        &self,
        kind: RuntimeOperationKind,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeObservation>;
}

/// Lists a bounded normalized installed-model inventory.
pub trait RuntimeModelInventoryProvider: Send + Sync {
    /// Returns at most `limit` normalized provider records.
    fn list_models(
        &self,
        limit: u16,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeModelInventory>;
}

/// Complete provider-neutral runtime adapter surface consumed by core policy.
pub trait RuntimeProvider:
    RuntimeDetector
    + RuntimeLifecycle
    + RuntimeModelInventoryProvider
    + RuntimeModelSetupProvider
    + Send
    + Sync
{
    /// Returns the stable identity handled by this provider instance.
    fn provider_id(&self) -> &RuntimeProviderId;
}
