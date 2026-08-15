use std::io::ErrorKind;

use thiserror::Error;

/// Internal adapter failure. Messages are intentionally safe for bounded diagnostics.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum OllamaAdapterError {
    #[error("the configured provider endpoint is invalid")]
    InvalidEndpoint,
    #[error("the configured provider endpoint is not loopback-only")]
    UnsafeEndpoint,
    #[error("the discovered provider executable path is invalid")]
    InvalidExecutable,
    #[error("the provider executable identity could not be trusted")]
    ExecutableUntrusted,
    #[error("provider executable inspection failed during {operation}: {kind:?}")]
    ExecutableIo {
        operation: &'static str,
        kind: ErrorKind,
    },
    #[error("the provider could not be reached on the approved loopback endpoint")]
    ConnectionFailed,
    #[error("the provider returned HTTP status {status}")]
    HttpStatus { status: u16 },
    #[error("the provider response exceeded the configured size limit")]
    ResponseTooLarge,
    #[error("the provider response did not match the required contract")]
    InvalidResponse,
    #[error("the provider version is invalid")]
    InvalidVersion,
    #[error("the provider operation timed out")]
    TimedOut,
    #[error("the provider operation was cancelled")]
    Cancelled,
    #[error("provider model acquisition did not reach a validated terminal success")]
    ModelAcquisitionFailed,
    #[error("provider model storage is unavailable")]
    StorageUnavailable,
    #[error("the fixed provider readiness inference failed")]
    ReadinessFailed,
    #[error("the requested lifecycle operation is unsupported for this installation")]
    LifecycleUnsupported,
    #[error("the requested lifecycle operation conflicts with current owned-process state")]
    LifecycleConflict,
    #[error("the owned provider child could not be started")]
    ProcessStartFailed,
    #[error("the owned provider child could not be controlled")]
    ProcessControlFailed,
    #[error("the provider command output exceeded the configured size limit")]
    ProcessOutputTooLarge,
    #[error("the provider adapter failed internally")]
    Internal,
}

impl OllamaAdapterError {
    pub(crate) fn executable_io(operation: &'static str, error: &std::io::Error) -> Self {
        Self::ExecutableIo {
            operation,
            kind: error.kind(),
        }
    }
}
