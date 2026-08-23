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
mod install;
mod model_setup;
mod provider;

#[cfg(any(test, feature = "test-support"))]
pub mod testing;
#[cfg(any(test, feature = "test-support"))]
mod testing_install;

pub use chat::{
    CHAT_DELTA_CHANNEL_CAPACITY, ChatDeltaSender, RuntimeChatMessage, RuntimeChatProvider,
    RuntimeChatRequest, RuntimeChatRole, RuntimeGenerationDelta, RuntimeGenerationResult,
};
pub use context::{RuntimeCancellationToken, RuntimeOperationContext};
pub use error::RuntimeError;
pub use install::{
    INSTALL_PROGRESS_CHANNEL_CAPACITY, InstallProgressSender, RuntimeArtifactOutcome,
    RuntimeArtifactRejection, RuntimeInstallCandidate, RuntimeInstallExecution,
    RuntimeInstallRefusal, RuntimeInstaller,
};
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
#[cfg(any(test, feature = "test-support"))]
pub use testing_install::{FakeInstallCall, FakeRuntimeInstaller, sample_candidate};
