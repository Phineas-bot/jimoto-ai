use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use gixgiz_contracts::{CorrelationId, RequestId};
use tokio::sync::Notify;

use crate::RuntimeError;

/// Cloneable cooperative cancellation signal for runtime operations.
#[derive(Clone, Debug, Default)]
pub struct RuntimeCancellationToken {
    inner: Arc<CancellationInner>,
}

#[derive(Debug, Default)]
struct CancellationInner {
    cancelled: AtomicBool,
    notify: Notify,
}

impl RuntimeCancellationToken {
    /// Creates an active cancellation token.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation and wakes awaiting provider work.
    pub fn cancel(&self) {
        if !self.inner.cancelled.swap(true, Ordering::AcqRel) {
            self.inner.notify.notify_waiters();
        }
    }

    /// Returns whether cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::Acquire)
    }

    /// Waits until cancellation is requested without losing an early signal.
    pub async fn cancelled(&self) {
        loop {
            if self.is_cancelled() {
                return;
            }
            let notified = self.inner.notify.notified();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }
}

/// Correlation, request, cancellation, and deadline control for provider work.
#[derive(Clone, Debug)]
pub struct RuntimeOperationContext {
    correlation_id: CorrelationId,
    request_id: RequestId,
    cancellation: RuntimeCancellationToken,
    deadline: Instant,
}

impl RuntimeOperationContext {
    /// Creates a bounded context from caller-owned identifiers.
    #[must_use]
    pub fn new(correlation_id: CorrelationId, request_id: RequestId, timeout: Duration) -> Self {
        let now = Instant::now();
        Self {
            correlation_id,
            request_id,
            cancellation: RuntimeCancellationToken::new(),
            deadline: now.checked_add(timeout).unwrap_or(now),
        }
    }

    /// Uses an existing cancellation signal, such as a host operation token.
    #[must_use]
    pub fn with_cancellation(mut self, cancellation: RuntimeCancellationToken) -> Self {
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

    /// Returns a clone of the cooperative cancellation signal.
    #[must_use]
    pub fn cancellation(&self) -> RuntimeCancellationToken {
        self.cancellation.clone()
    }

    /// Returns the remaining bounded duration, or zero after the deadline.
    #[must_use]
    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    /// Checks cancellation before timeout so explicit user intent wins.
    pub fn check(&self) -> Result<(), RuntimeError> {
        if self.cancellation.is_cancelled() {
            return Err(RuntimeError::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(RuntimeError::TimedOut);
        }
        Ok(())
    }

    /// Runs a future within the operation deadline and cancellation signal.
    pub async fn run<T>(
        &self,
        future: impl std::future::Future<Output = Result<T, RuntimeError>>,
    ) -> Result<T, RuntimeError> {
        self.check()?;
        tokio::select! {
            biased;
            () = self.cancellation.cancelled() => Err(RuntimeError::Cancelled),
            result = tokio::time::timeout(self.remaining(), future) => {
                result.unwrap_or(Err(RuntimeError::TimedOut))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancellation_wins_and_wakes_waiters() {
        let token = RuntimeCancellationToken::new();
        let context = RuntimeOperationContext::new(
            CorrelationId::new(),
            RequestId::new(),
            Duration::from_secs(1),
        )
        .with_cancellation(token.clone());
        token.cancel();

        assert_eq!(
            context.run(async { Ok::<_, RuntimeError>(()) }).await,
            Err(RuntimeError::Cancelled)
        );
    }

    #[tokio::test]
    async fn zero_deadline_is_timed_out() {
        let context =
            RuntimeOperationContext::new(CorrelationId::new(), RequestId::new(), Duration::ZERO);

        assert_eq!(context.check(), Err(RuntimeError::TimedOut));
    }
}
