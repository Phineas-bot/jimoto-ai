//! Provider-neutral local runtime abstraction for GixGiz.
//!
//! This crate defines provider traits, bounded operation control, normalized
//! observations, typed failures, and deterministic test support. It contains
//! no provider routes, commands, executable paths, transport, persistence SQL,
//! or Flutter concerns.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod context;
mod error;
mod provider;

#[cfg(any(test, feature = "test-support"))]
pub mod testing;

pub use context::{RuntimeCancellationToken, RuntimeOperationContext};
pub use error::RuntimeError;
pub use provider::{
    RuntimeDetector, RuntimeFuture, RuntimeLifecycle, RuntimeModelInventoryProvider,
    RuntimeObservation, RuntimeProvider,
};
