//! Platform lifecycle and provider-neutral readiness policy for GixGiz.
//!
//! This crate owns core application behavior, composes Rust-owned persistence,
//! and consumes shared contracts. The first non-elevated Windows hardware
//! provider remains behind a provider-neutral trait in this crate. This crate
//! does not own transport or Flutter presentation; the desktop host adapts its
//! services to the authenticated local boundary.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod capability;
mod chat;
mod error;
mod hardware;
mod observability;
mod operation;
mod persistence;
mod runtime;
mod runtime_install;
mod service;
mod setup;
mod storage_location;

pub use capability::CapabilityEngine;
pub use chat::{ChatEventSubscription, ChatService};
pub use error::CoreError;
pub use hardware::{
    CollectedHardwareEvidence, HardwareProvider, HardwareScanner, WindowsHardwareProvider,
};
pub use observability::{TracingInitError, init_tracing};
pub use operation::{CancellationToken, OperationContext};
pub use runtime::{RuntimeService, runtime_is_usable};
pub use runtime_install::{
    MAX_INSTALL_RETRIES, RuntimeInstallCoordinator, RuntimeInstallJob, RuntimeInstallService,
    decision_authorizes_work,
};
pub use service::{CoreLifecycle, PlatformCore, ServiceHealthSource, compose_readiness};
pub use setup::SetupService;
pub use storage_location::{
    FreeSpaceProbe, SetOutcome, StorageLocationService, WindowsFreeSpaceProbe,
};
