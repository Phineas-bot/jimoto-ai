use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use gixgiz_contracts::{CorrelationId, RequestId};

use crate::CoreError;

/// Cloneable cooperative-cancellation signal for current and future jobs.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Creates an active cancellation token.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks the shared operation as cancelled.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Returns whether cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

/// IDs, cancellation, and an optional monotonic deadline for one operation.
#[derive(Clone, Debug)]
pub struct OperationContext {
    correlation_id: CorrelationId,
    request_id: RequestId,
    cancellation: CancellationToken,
    deadline: Option<Instant>,
}

impl OperationContext {
    /// Creates a context with fresh opaque identifiers and no deadline.
    #[must_use]
    pub fn generated() -> Self {
        Self::new(CorrelationId::new(), RequestId::new())
    }

    /// Creates a deterministic context from caller-supplied identifiers.
    #[must_use]
    pub fn new(correlation_id: CorrelationId, request_id: RequestId) -> Self {
        Self {
            correlation_id,
            request_id,
            cancellation: CancellationToken::new(),
            deadline: None,
        }
    }

    /// Adds a monotonic timeout, failing closed if the duration overflows.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        let now = Instant::now();
        self.deadline = Some(now.checked_add(timeout).unwrap_or(now));
        self
    }

    /// Uses a caller-owned cancellation signal for a hosted operation.
    #[must_use]
    pub fn with_cancellation(mut self, cancellation: CancellationToken) -> Self {
        self.cancellation = cancellation;
        self
    }

    /// Returns the operation correlation identifier.
    #[must_use]
    pub const fn correlation_id(&self) -> CorrelationId {
        self.correlation_id
    }

    /// Returns the request identifier.
    #[must_use]
    pub const fn request_id(&self) -> RequestId {
        self.request_id
    }

    /// Returns a clone of the cooperative cancellation token.
    #[must_use]
    pub fn cancellation(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    /// Checks cancellation before timeout so an explicit user action wins.
    pub fn check(&self) -> Result<(), CoreError> {
        if self.cancellation.is_cancelled() {
            return Err(CoreError::Cancelled);
        }

        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Err(CoreError::TimedOut);
        }

        Ok(())
    }
}
