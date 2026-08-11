use std::io;

use gixgiz_core::CoreError;
use gixgiz_runtime::RuntimeError;
use thiserror::Error;

/// Safe top-level failures produced while hosting the desktop sidecar.
#[derive(Debug, Error)]
pub enum HostError {
    /// The inherited bootstrap record did not arrive before the deadline.
    #[error("the desktop bootstrap timed out")]
    BootstrapTimedOut,
    /// The bootstrap pipe closed before one complete record arrived.
    #[error("the desktop bootstrap pipe closed before initialization")]
    BootstrapClosed,
    /// The bootstrap record exceeded the fixed maximum size.
    #[error("the desktop bootstrap record exceeded its size limit")]
    BootstrapTooLarge,
    /// The bootstrap record was malformed or failed validation.
    #[error("the desktop bootstrap record was invalid")]
    InvalidBootstrap,
    /// The public bootstrap response could not be returned to the desktop.
    #[error("the core could not return its bootstrap response")]
    BootstrapResponse,
    /// The loopback-only listener could not be created.
    #[error("the core could not bind its local endpoint")]
    Bind(#[source] io::Error),
    /// The local HTTP server stopped because of an I/O failure.
    #[error("the core local endpoint stopped unexpectedly")]
    Serve(#[source] io::Error),
    /// The platform core failed during startup or shutdown.
    #[error("the platform core lifecycle operation failed")]
    Core(#[source] CoreError),
    /// A blocking core lifecycle worker stopped unexpectedly.
    #[error("the platform core lifecycle worker stopped unexpectedly")]
    CoreWorker(#[source] tokio::task::JoinError),
    /// The concrete runtime adapter could not be composed safely.
    #[error("the local runtime adapter could not be initialized")]
    Runtime(#[source] RuntimeError),
}
