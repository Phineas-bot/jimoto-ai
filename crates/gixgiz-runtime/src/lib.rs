//! Provider-neutral local runtime abstraction for GixGiz.
//!
//! This crate defines provider traits, bounded operation control, normalized
//! observations, typed failures, and deterministic test support. It contains
//! no provider routes, commands, executable paths, transport, persistence SQL,
//! or Flutter concerns.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod chat;
mod context;
mod error;
mod model_setup;
mod provider;

#[cfg(any(test, feature = "test-support"))]
pub mod testing;

pub use chat::{
    CHAT_DELTA_CHANNEL_CAPACITY, ChatDeltaSender, RuntimeChatMessage, RuntimeChatProvider,
    RuntimeChatRequest, RuntimeChatRole, RuntimeGenerationDelta, RuntimeGenerationResult,
};
pub use context::{RuntimeCancellationToken, RuntimeOperationContext};
pub use error::RuntimeError;
pub use model_setup::{
    MODEL_PROGRESS_CHANNEL_CAPACITY, ModelProgressSender, RuntimeCancellationSemantics,
    RuntimeModelAcquisitionPlan, RuntimeModelAcquisitionResult, RuntimeModelAcquisitionStatus,
    RuntimeModelInspection, RuntimeModelSetupProvider, RuntimeReadinessInferenceResult,
    RuntimeStorageAvailability, RuntimeStoragePreflight,
};
pub use provider::{
    RuntimeDetector, RuntimeFuture, RuntimeLifecycle, RuntimeModelInventoryProvider,
    RuntimeObservation, RuntimeProvider,
};
