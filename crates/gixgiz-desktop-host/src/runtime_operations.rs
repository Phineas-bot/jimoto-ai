use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use gixgiz_contracts::{
    CorrelationId, OperationId, RUNTIME_REPORT_SCHEMA_VERSION, RequestId, RuntimeOperationEvent,
    RuntimeOperationEventKind, RuntimeOperationKind, RuntimeOperationTerminalState,
    RuntimeProviderId, SafeErrorPayload,
};
use gixgiz_core::RuntimeService;
use gixgiz_runtime::{RuntimeCancellationToken, RuntimeError, RuntimeOperationContext};
use tokio::sync::broadcast;

use crate::operations::OperationError;

const EVENT_CHANNEL_CAPACITY: usize = 8;
const MAX_RETAINED_OPERATIONS: usize = 8;
const LIFECYCLE_TIMEOUT: Duration = Duration::from_secs(30);
const TERMINAL_STATUS_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone)]
pub(crate) struct RuntimeOperationRegistry {
    service: RuntimeService,
    records: Arc<Mutex<HashMap<OperationId, RuntimeOperationRecord>>>,
}

struct RuntimeOperationRecord {
    correlation_id: CorrelationId,
    cancellation: RuntimeCancellationToken,
    events: Vec<RuntimeOperationEvent>,
    sender: broadcast::Sender<RuntimeOperationEvent>,
    terminal: bool,
}

pub(crate) struct RuntimeOperationSubscription {
    pub(crate) replay: Vec<RuntimeOperationEvent>,
    pub(crate) receiver: broadcast::Receiver<RuntimeOperationEvent>,
    pub(crate) terminal: bool,
}

impl RuntimeOperationRegistry {
    pub(crate) fn new(service: RuntimeService) -> Self {
        Self {
            service,
            records: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) fn start(
        &self,
        provider_id: RuntimeProviderId,
        kind: RuntimeOperationKind,
        correlation_id: CorrelationId,
        request_id: RequestId,
    ) -> Result<OperationId, OperationError> {
        let operation_id = OperationId::new();
        let cancellation = RuntimeCancellationToken::new();
        let (sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let started = event(
            operation_id,
            correlation_id,
            1,
            kind,
            RuntimeOperationEventKind::Started,
            Some("The runtime lifecycle operation started."),
            None,
            None,
            None,
        );
        {
            let mut records = self.lock()?;
            if records.values().any(|record| !record.terminal) {
                return Err(OperationError::Busy);
            }
            if records.len() >= MAX_RETAINED_OPERATIONS
                && let Some(id) = records
                    .iter()
                    .find_map(|(id, record)| record.terminal.then_some(*id))
            {
                records.remove(&id);
            }
            records.insert(
                operation_id,
                RuntimeOperationRecord {
                    correlation_id,
                    cancellation: cancellation.clone(),
                    events: vec![started],
                    sender,
                    terminal: false,
                },
            );
        }

        let registry = self.clone();
        tokio::spawn(async move {
            registry
                .run(
                    operation_id,
                    provider_id,
                    kind,
                    correlation_id,
                    request_id,
                    cancellation,
                )
                .await;
        });
        Ok(operation_id)
    }

    pub(crate) fn subscribe(
        &self,
        operation_id: OperationId,
        correlation_id: CorrelationId,
    ) -> Result<RuntimeOperationSubscription, OperationError> {
        let records = self.lock()?;
        let record = records.get(&operation_id).ok_or(OperationError::NotFound)?;
        if record.correlation_id != correlation_id {
            return Err(OperationError::CorrelationMismatch);
        }
        Ok(RuntimeOperationSubscription {
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

    async fn run(
        &self,
        operation_id: OperationId,
        provider_id: RuntimeProviderId,
        kind: RuntimeOperationKind,
        correlation_id: CorrelationId,
        request_id: RequestId,
        cancellation: RuntimeCancellationToken,
    ) {
        if self
            .push(
                operation_id,
                RuntimeOperationEventKind::Progress,
                Some("Applying runtime ownership and consent policy."),
                None,
                None,
                None,
            )
            .is_err()
        {
            return;
        }
        let context = RuntimeOperationContext::new(correlation_id, request_id, LIFECYCLE_TIMEOUT)
            .with_cancellation(cancellation);
        let result = self
            .service
            .execute(&provider_id, kind, context.clone())
            .await;
        match result {
            Ok(report) => {
                let _ = self.push(
                    operation_id,
                    RuntimeOperationEventKind::Completed,
                    Some("The runtime lifecycle operation completed."),
                    Some(report),
                    None,
                    Some(RuntimeOperationTerminalState::Completed),
                );
            }
            Err(RuntimeError::Cancelled) => {
                let report = self
                    .terminal_report(&provider_id, correlation_id, request_id)
                    .await;
                let _ = self.push(
                    operation_id,
                    RuntimeOperationEventKind::Cancelled,
                    Some("The runtime lifecycle operation was cancelled."),
                    report,
                    None,
                    Some(RuntimeOperationTerminalState::Cancelled),
                );
            }
            Err(RuntimeError::TimedOut) => {
                let error = RuntimeError::TimedOut.to_safe_payload(&context);
                let report = self
                    .terminal_report(&provider_id, correlation_id, request_id)
                    .await;
                let _ = self.push(
                    operation_id,
                    RuntimeOperationEventKind::TimedOut,
                    Some("The runtime lifecycle operation timed out."),
                    report,
                    Some(error),
                    Some(RuntimeOperationTerminalState::TimedOut),
                );
            }
            Err(error) => {
                let payload = error.to_safe_payload(&context);
                let report = self
                    .terminal_report(&provider_id, correlation_id, request_id)
                    .await;
                let _ = self.push(
                    operation_id,
                    RuntimeOperationEventKind::Failed,
                    Some("The runtime lifecycle operation failed safely."),
                    report,
                    Some(payload),
                    Some(RuntimeOperationTerminalState::Failed),
                );
            }
        }
    }

    async fn terminal_report(
        &self,
        provider_id: &RuntimeProviderId,
        correlation_id: CorrelationId,
        request_id: RequestId,
    ) -> Option<gixgiz_contracts::RuntimeHealthReport> {
        let context =
            RuntimeOperationContext::new(correlation_id, request_id, TERMINAL_STATUS_TIMEOUT);
        self.service.status(provider_id, context).await.ok()
    }

    fn push(
        &self,
        operation_id: OperationId,
        kind: RuntimeOperationEventKind,
        message: Option<&str>,
        report: Option<gixgiz_contracts::RuntimeHealthReport>,
        error: Option<SafeErrorPayload>,
        terminal_state: Option<RuntimeOperationTerminalState>,
    ) -> Result<(), OperationError> {
        let mut records = self.lock()?;
        let record = records
            .get_mut(&operation_id)
            .ok_or(OperationError::NotFound)?;
        if record.terminal {
            return Ok(());
        }
        let operation_kind = record
            .events
            .first()
            .map_or(RuntimeOperationKind::Unknown, |item| item.operation_kind);
        let next = event(
            operation_id,
            record.correlation_id,
            record.events.len() as u64 + 1,
            operation_kind,
            kind,
            message,
            report,
            error,
            terminal_state,
        );
        record.terminal = terminal_state.is_some();
        record.events.push(next.clone());
        let _ = record.sender.send(next);
        Ok(())
    }

    fn lock(
        &self,
    ) -> Result<MutexGuard<'_, HashMap<OperationId, RuntimeOperationRecord>>, OperationError> {
        self.records.lock().map_err(|_| OperationError::Internal)
    }
}

#[allow(clippy::too_many_arguments)]
fn event(
    operation_id: OperationId,
    correlation_id: CorrelationId,
    sequence: u64,
    operation_kind: RuntimeOperationKind,
    kind: RuntimeOperationEventKind,
    message: Option<&str>,
    report: Option<gixgiz_contracts::RuntimeHealthReport>,
    error: Option<SafeErrorPayload>,
    terminal_state: Option<RuntimeOperationTerminalState>,
) -> RuntimeOperationEvent {
    RuntimeOperationEvent {
        schema_version: RUNTIME_REPORT_SCHEMA_VERSION,
        operation_id,
        correlation_id,
        sequence,
        operation_kind,
        kind,
        timestamp_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| {
                u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
            }),
        message: message.map(ToOwned::to_owned),
        report,
        error,
        terminal_state,
    }
}
