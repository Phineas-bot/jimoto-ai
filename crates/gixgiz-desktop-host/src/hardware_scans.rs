use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use gixgiz_contracts::{
    CorrelationId, HardwareScanEvent, HardwareScanEventKind, HardwareScanTerminalState,
    MachineProfile, MachineProfileCompleteness, OperationId, RequestId, SCHEMA_VERSION,
    SafeErrorPayload,
};
use gixgiz_core::{CancellationToken, CoreError, HardwareScanner, OperationContext};
use tokio::sync::broadcast;

use crate::operations::OperationError;

const EVENT_CHANNEL_CAPACITY: usize = 8;
const MAX_RETAINED_SCANS: usize = 8;
const SCAN_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone)]
pub(crate) struct HardwareScanRegistry {
    scanner: HardwareScanner,
    records: Arc<Mutex<HashMap<OperationId, HardwareScanRecord>>>,
}

struct HardwareScanRecord {
    correlation_id: CorrelationId,
    cancellation: CancellationToken,
    events: Vec<HardwareScanEvent>,
    sender: broadcast::Sender<HardwareScanEvent>,
    terminal: bool,
}

pub(crate) struct HardwareScanSubscription {
    pub(crate) replay: Vec<HardwareScanEvent>,
    pub(crate) receiver: broadcast::Receiver<HardwareScanEvent>,
    pub(crate) terminal: bool,
}

impl HardwareScanRegistry {
    pub(crate) fn new(scanner: HardwareScanner) -> Self {
        Self {
            scanner,
            records: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) fn start(
        &self,
        correlation_id: CorrelationId,
        request_id: RequestId,
    ) -> Result<OperationId, OperationError> {
        let operation_id = OperationId::new();
        let cancellation = CancellationToken::new();
        let (sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let started = event(
            operation_id,
            correlation_id,
            1,
            HardwareScanEventKind::Started,
            Some("Hardware evidence scan started."),
            None,
            None,
            None,
        );
        {
            let mut records = self.lock()?;
            if records.values().any(|record| !record.terminal) {
                return Err(OperationError::Busy);
            }
            if records.len() >= MAX_RETAINED_SCANS {
                let completed = records
                    .iter()
                    .find_map(|(id, record)| record.terminal.then_some(*id));
                if let Some(id) = completed {
                    records.remove(&id);
                }
            }
            records.insert(
                operation_id,
                HardwareScanRecord {
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
                .run(operation_id, correlation_id, request_id, cancellation)
                .await;
        });
        Ok(operation_id)
    }

    pub(crate) fn subscribe(
        &self,
        operation_id: OperationId,
        correlation_id: CorrelationId,
    ) -> Result<HardwareScanSubscription, OperationError> {
        let records = self.lock()?;
        let record = records.get(&operation_id).ok_or(OperationError::NotFound)?;
        if record.correlation_id != correlation_id {
            return Err(OperationError::CorrelationMismatch);
        }
        Ok(HardwareScanSubscription {
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
        correlation_id: CorrelationId,
        request_id: RequestId,
        cancellation: CancellationToken,
    ) {
        if self
            .push(
                operation_id,
                HardwareScanEventKind::Collecting,
                Some("Collecting bounded Windows hardware evidence."),
                None,
                None,
                None,
            )
            .is_err()
        {
            return;
        }

        let context = OperationContext::new(correlation_id, request_id)
            .with_cancellation(cancellation)
            .with_timeout(SCAN_TIMEOUT);
        let worker_context = context.clone();
        let scanner = self.scanner.clone();
        let result =
            tokio::task::spawn_blocking(move || scanner.scan(operation_id, &worker_context))
                .await
                .unwrap_or(Err(CoreError::HardwareProviderUnavailable));

        match result {
            Ok(profile) => {
                let terminal = match profile.completeness {
                    MachineProfileCompleteness::Complete => HardwareScanTerminalState::Completed,
                    _ => HardwareScanTerminalState::Partial,
                };
                let message = if terminal == HardwareScanTerminalState::Completed {
                    "Hardware evidence scan completed."
                } else {
                    "Hardware evidence scan completed with explicit unknown values."
                };
                let _ = self.push(
                    operation_id,
                    HardwareScanEventKind::Completed,
                    Some(message),
                    Some(profile),
                    None,
                    Some(terminal),
                );
            }
            Err(CoreError::Cancelled) => {
                let _ = self.push(
                    operation_id,
                    HardwareScanEventKind::Cancelled,
                    Some("Hardware evidence scan was cancelled."),
                    None,
                    None,
                    Some(HardwareScanTerminalState::Cancelled),
                );
            }
            Err(CoreError::TimedOut) => {
                let _ = self.push(
                    operation_id,
                    HardwareScanEventKind::TimedOut,
                    Some("Hardware evidence scan timed out."),
                    None,
                    Some(CoreError::TimedOut.to_safe_payload(&context)),
                    Some(HardwareScanTerminalState::TimedOut),
                );
            }
            Err(error) => {
                let payload = error.to_safe_payload(&context);
                let _ = self.push(
                    operation_id,
                    HardwareScanEventKind::Failed,
                    Some("Hardware evidence scan failed safely."),
                    None,
                    Some(payload),
                    Some(HardwareScanTerminalState::Failed),
                );
            }
        }
    }

    fn push(
        &self,
        operation_id: OperationId,
        kind: HardwareScanEventKind,
        message: Option<&str>,
        profile: Option<MachineProfile>,
        error: Option<SafeErrorPayload>,
        terminal_state: Option<HardwareScanTerminalState>,
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
            profile,
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
    ) -> Result<MutexGuard<'_, HashMap<OperationId, HardwareScanRecord>>, OperationError> {
        self.records.lock().map_err(|_| OperationError::Internal)
    }
}

#[allow(clippy::too_many_arguments)]
fn event(
    operation_id: OperationId,
    correlation_id: CorrelationId,
    sequence: u64,
    kind: HardwareScanEventKind,
    message: Option<&str>,
    profile: Option<MachineProfile>,
    error: Option<SafeErrorPayload>,
    terminal_state: Option<HardwareScanTerminalState>,
) -> HardwareScanEvent {
    HardwareScanEvent {
        schema_version: SCHEMA_VERSION,
        operation_id,
        correlation_id,
        sequence,
        kind,
        timestamp_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis() as u64),
        message: message.map(ToOwned::to_owned),
        profile,
        error,
        terminal_state,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use gixgiz_contracts::{
        AccelerationEvidence, AccelerationKind, ArchitectureEvidence, CpuEvidence,
        EvidenceConfidence, EvidenceMetadata, EvidenceSource, GpuCollectionEvidence,
        MachineArchitecture, OperatingSystemEvidence, PhysicalMemoryEvidence, StorageEvidence,
        StorageLocation, StorageMediaEvidence, StorageMediaKind, StringEvidence, U32Evidence,
        U64Evidence,
    };
    use gixgiz_core::{CollectedHardwareEvidence, HardwareProvider};

    use super::*;

    struct FakeProvider {
        delay: Duration,
    }

    impl HardwareProvider for FakeProvider {
        fn collect(
            &self,
            context: &OperationContext,
        ) -> Result<CollectedHardwareEvidence, CoreError> {
            let started = std::time::Instant::now();
            while started.elapsed() < self.delay {
                context.check()?;
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok(evidence())
        }
    }

    #[tokio::test]
    async fn scan_events_are_ordered_and_include_a_profile() {
        let registry = HardwareScanRegistry::new(HardwareScanner::new(Arc::new(FakeProvider {
            delay: Duration::ZERO,
        })));
        let correlation_id = CorrelationId::new();
        let operation_id = registry
            .start(correlation_id, RequestId::new())
            .expect("scan starts");
        tokio::time::sleep(Duration::from_millis(50)).await;
        let subscription = registry
            .subscribe(operation_id, correlation_id)
            .expect("scan exists");

        assert!(subscription.terminal);
        assert_eq!(
            subscription
                .replay
                .iter()
                .map(|item| item.sequence)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        let terminal = subscription.replay.last().expect("terminal event exists");
        assert_eq!(
            terminal.terminal_state,
            Some(HardwareScanTerminalState::Completed)
        );
        assert_eq!(
            terminal.profile.as_ref().map(|profile| profile.scan_id),
            Some(operation_id)
        );
    }

    #[tokio::test]
    async fn cancellation_reaches_the_blocking_provider() {
        let registry = HardwareScanRegistry::new(HardwareScanner::new(Arc::new(FakeProvider {
            delay: Duration::from_secs(1),
        })));
        let correlation_id = CorrelationId::new();
        let operation_id = registry
            .start(correlation_id, RequestId::new())
            .expect("scan starts");
        assert!(
            registry
                .cancel(operation_id, correlation_id)
                .expect("cancel accepted")
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
        let subscription = registry
            .subscribe(operation_id, correlation_id)
            .expect("scan exists");

        assert_eq!(
            subscription
                .replay
                .last()
                .and_then(|item| item.terminal_state),
            Some(HardwareScanTerminalState::Cancelled)
        );
    }

    fn evidence() -> CollectedHardwareEvidence {
        let metadata =
            EvidenceMetadata::available(EvidenceSource::WindowsCim, EvidenceConfidence::High);
        let text = |value: &str| StringEvidence {
            value: Some(value.to_owned()),
            metadata: metadata.clone(),
        };
        CollectedHardwareEvidence {
            operating_system: OperatingSystemEvidence {
                name: text("Windows"),
                version: text("10.0"),
                build: text("26100"),
                architecture: ArchitectureEvidence {
                    value: Some(MachineArchitecture::X86_64),
                    metadata: metadata.clone(),
                },
            },
            cpu: CpuEvidence {
                name: text("CPU"),
                vendor: text("Vendor"),
                physical_core_count: U32Evidence {
                    value: Some(4),
                    metadata: metadata.clone(),
                },
                logical_core_count: U32Evidence {
                    value: Some(8),
                    metadata: metadata.clone(),
                },
            },
            physical_memory: PhysicalMemoryEvidence {
                total_bytes: U64Evidence {
                    value: Some(16),
                    metadata: metadata.clone(),
                },
                available_bytes: U64Evidence {
                    value: Some(8),
                    metadata: metadata.clone(),
                },
            },
            gpus: GpuCollectionEvidence {
                devices: Vec::new(),
                metadata: metadata.clone(),
            },
            acceleration: vec![AccelerationEvidence {
                kind: AccelerationKind::DirectMl,
                supported: Some(true),
                metadata: metadata.clone(),
            }],
            storage: StorageEvidence {
                location: StorageLocation::ApplicationData,
                capacity_bytes: U64Evidence {
                    value: Some(100),
                    metadata: metadata.clone(),
                },
                free_bytes: U64Evidence {
                    value: Some(50),
                    metadata: metadata.clone(),
                },
                filesystem: text("NTFS"),
                media_kind: StorageMediaEvidence {
                    value: Some(StorageMediaKind::Fixed),
                    metadata,
                },
            },
        }
    }
}
