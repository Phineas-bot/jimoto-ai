use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use gixgiz_contracts::{
    CorrelationId, OperationId, SCHEMA_VERSION, TestOperationEvent, TestOperationEventKind,
    TestOperationTerminalState,
};
use gixgiz_core::CancellationToken;
use tokio::sync::broadcast;

const EVENT_CHANNEL_CAPACITY: usize = 16;
const PROGRESS_STEPS: u64 = 3;
const STEP_DELAY: Duration = Duration::from_millis(75);

#[derive(Clone, Default)]
pub(crate) struct OperationRegistry {
    records: Arc<Mutex<HashMap<OperationId, OperationRecord>>>,
}

struct OperationRecord {
    correlation_id: CorrelationId,
    cancellation: CancellationToken,
    events: Vec<TestOperationEvent>,
    sender: broadcast::Sender<TestOperationEvent>,
    terminal: bool,
}

pub(crate) struct OperationSubscription {
    pub(crate) replay: Vec<TestOperationEvent>,
    pub(crate) receiver: broadcast::Receiver<TestOperationEvent>,
    pub(crate) terminal: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OperationError {
    NotFound,
    CorrelationMismatch,
    Busy,
    Internal,
}

impl OperationRegistry {
    pub(crate) fn start(
        &self,
        correlation_id: CorrelationId,
    ) -> Result<OperationId, OperationError> {
        let operation_id = OperationId::new();
        let cancellation = CancellationToken::new();
        let (sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let started = event(
            operation_id,
            correlation_id,
            1,
            TestOperationEventKind::Started,
            Some("The deterministic transport operation started."),
            None,
        );
        let record = OperationRecord {
            correlation_id,
            cancellation,
            events: vec![started],
            sender,
            terminal: false,
        };
        self.lock()?.insert(operation_id, record);

        let registry = self.clone();
        tokio::spawn(async move {
            registry.run(operation_id).await;
        });
        Ok(operation_id)
    }

    pub(crate) fn subscribe(
        &self,
        operation_id: OperationId,
    ) -> Result<OperationSubscription, OperationError> {
        let records = self.lock()?;
        let record = records.get(&operation_id).ok_or(OperationError::NotFound)?;
        Ok(OperationSubscription {
            replay: record.events.clone(),
            receiver: record.sender.subscribe(),
            terminal: record.terminal,
        })
    }

    pub(crate) fn cancel(
        &self,
        operation_id: OperationId,
        correlation_id: CorrelationId,
    ) -> Result<bool, OperationError> {
        let records = self.lock()?;
        let record = records.get(&operation_id).ok_or(OperationError::NotFound)?;
        if record.correlation_id != correlation_id {
            return Err(OperationError::CorrelationMismatch);
        }
        if record.terminal {
            return Ok(false);
        }
        record.cancellation.cancel();
        Ok(true)
    }

    async fn run(&self, operation_id: OperationId) {
        for step in 1..=PROGRESS_STEPS {
            tokio::time::sleep(STEP_DELAY).await;
            let cancellation = match self.cancellation(operation_id) {
                Ok(cancellation) => cancellation,
                Err(_) => return,
            };
            if cancellation.is_cancelled() {
                let _ = self.push_terminal(
                    operation_id,
                    TestOperationEventKind::Cancelled,
                    TestOperationTerminalState::Cancelled,
                    "The deterministic transport operation was cancelled.",
                );
                return;
            }
            if self
                .push_progress(
                    operation_id,
                    &format!("Deterministic progress step {step} of {PROGRESS_STEPS}."),
                )
                .is_err()
            {
                return;
            }
        }

        let _ = self.push_terminal(
            operation_id,
            TestOperationEventKind::Completed,
            TestOperationTerminalState::Completed,
            "The deterministic transport operation completed.",
        );
    }

    fn cancellation(&self, operation_id: OperationId) -> Result<CancellationToken, OperationError> {
        self.lock()?
            .get(&operation_id)
            .map(|record| record.cancellation.clone())
            .ok_or(OperationError::NotFound)
    }

    fn push_progress(
        &self,
        operation_id: OperationId,
        message: &str,
    ) -> Result<(), OperationError> {
        self.push(
            operation_id,
            TestOperationEventKind::Progress,
            Some(message),
            None,
        )
    }

    fn push_terminal(
        &self,
        operation_id: OperationId,
        kind: TestOperationEventKind,
        terminal_state: TestOperationTerminalState,
        message: &str,
    ) -> Result<(), OperationError> {
        self.push(operation_id, kind, Some(message), Some(terminal_state))
    }

    fn push(
        &self,
        operation_id: OperationId,
        kind: TestOperationEventKind,
        message: Option<&str>,
        terminal_state: Option<TestOperationTerminalState>,
    ) -> Result<(), OperationError> {
        let mut records = self.lock()?;
        let record = records
            .get_mut(&operation_id)
            .ok_or(OperationError::NotFound)?;
        if record.terminal {
            return Ok(());
        }
        let next = event(
            operation_id,
            record.correlation_id,
            record.events.len() as u64 + 1,
            kind,
            message,
            terminal_state,
        );
        record.terminal = terminal_state.is_some();
        record.events.push(next.clone());
        let _ = record.sender.send(next);
        Ok(())
    }

    fn lock(
        &self,
    ) -> Result<MutexGuard<'_, HashMap<OperationId, OperationRecord>>, OperationError> {
        self.records.lock().map_err(|_| OperationError::Internal)
    }
}

fn event(
    operation_id: OperationId,
    correlation_id: CorrelationId,
    sequence: u64,
    kind: TestOperationEventKind,
    message: Option<&str>,
    terminal_state: Option<TestOperationTerminalState>,
) -> TestOperationEvent {
    TestOperationEvent {
        schema_version: SCHEMA_VERSION,
        operation_id,
        correlation_id,
        sequence,
        kind,
        timestamp_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis() as u64),
        message: message.map(ToOwned::to_owned),
        terminal_state,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn deterministic_events_are_ordered_and_complete() {
        let registry = OperationRegistry::default();
        let correlation_id = CorrelationId::new();
        let operation_id = registry.start(correlation_id).expect("operation starts");
        tokio::time::sleep(Duration::from_millis(350)).await;
        let subscription = registry.subscribe(operation_id).expect("operation exists");

        assert!(subscription.terminal);
        assert_eq!(
            subscription
                .replay
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5]
        );
        assert!(
            subscription
                .replay
                .iter()
                .all(|event| event.correlation_id == correlation_id)
        );
        assert_eq!(
            subscription
                .replay
                .last()
                .and_then(|event| event.terminal_state),
            Some(TestOperationTerminalState::Completed)
        );
    }

    #[tokio::test]
    async fn explicit_cancellation_emits_cancelled_terminal_event() {
        let registry = OperationRegistry::default();
        let correlation_id = CorrelationId::new();
        let operation_id = registry.start(correlation_id).expect("operation starts");

        assert!(
            registry
                .cancel(operation_id, correlation_id)
                .expect("operation can be cancelled")
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
        let subscription = registry.subscribe(operation_id).expect("operation exists");

        assert_eq!(
            subscription
                .replay
                .last()
                .and_then(|event| event.terminal_state),
            Some(TestOperationTerminalState::Cancelled)
        );
    }
}
