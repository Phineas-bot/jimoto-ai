//! Provider-neutral hardware scan orchestration and Windows provider boundary.

mod windows;

use std::{sync::Arc, time::SystemTime};

use gixgiz_contracts::{
    AccelerationEvidence, CpuEvidence, GpuCollectionEvidence, MACHINE_PROFILE_SCHEMA_VERSION,
    MachineProfile, MachineProfileCompleteness, OperatingSystemEvidence, OperationId,
    PhysicalMemoryEvidence, StorageEvidence,
};

use crate::{CoreError, OperationContext};

pub use windows::WindowsHardwareProvider;

/// Provider output before operation identifiers and completion policy are applied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectedHardwareEvidence {
    /// Operating-system evidence.
    pub operating_system: OperatingSystemEvidence,
    /// CPU evidence.
    pub cpu: CpuEvidence,
    /// Physical-memory evidence.
    pub physical_memory: PhysicalMemoryEvidence,
    /// Display-adapter evidence.
    pub gpus: GpuCollectionEvidence,
    /// Acceleration evidence.
    pub acceleration: Vec<AccelerationEvidence>,
    /// Selected storage evidence.
    pub storage: StorageEvidence,
}

/// Operating-system-specific provider hidden behind the core scanner service.
pub trait HardwareProvider: Send + Sync {
    /// Collects bounded evidence while honoring cancellation and the deadline.
    fn collect(&self, context: &OperationContext) -> Result<CollectedHardwareEvidence, CoreError>;
}

/// Core-owned hardware scanner that applies operation and completeness policy.
#[derive(Clone)]
pub struct HardwareScanner {
    provider: Arc<dyn HardwareProvider>,
}

impl HardwareScanner {
    /// Creates a scanner with an injected platform provider.
    #[must_use]
    pub fn new(provider: Arc<dyn HardwareProvider>) -> Self {
        Self { provider }
    }

    /// Creates the supported non-elevated Windows scanner.
    pub fn windows() -> Result<Self, CoreError> {
        Ok(Self::new(Arc::new(WindowsHardwareProvider::new()?)))
    }

    /// Collects one versioned machine profile for the supplied operation.
    pub fn scan(
        &self,
        scan_id: OperationId,
        context: &OperationContext,
    ) -> Result<MachineProfile, CoreError> {
        context.check()?;
        let evidence = self.provider.collect(context)?;
        context.check()?;
        let completeness = completeness(&evidence);

        Ok(MachineProfile {
            schema_version: MACHINE_PROFILE_SCHEMA_VERSION,
            scan_id,
            correlation_id: context.correlation_id(),
            scanned_at_unix_ms: unix_timestamp_millis(),
            completeness,
            operating_system: evidence.operating_system,
            cpu: evidence.cpu,
            physical_memory: evidence.physical_memory,
            gpus: evidence.gpus,
            acceleration: evidence.acceleration,
            storage: evidence.storage,
        })
    }
}

fn completeness(evidence: &CollectedHardwareEvidence) -> MachineProfileCompleteness {
    let mut metadata = vec![
        &evidence.operating_system.name.metadata,
        &evidence.operating_system.version.metadata,
        &evidence.operating_system.build.metadata,
        &evidence.operating_system.architecture.metadata,
        &evidence.cpu.name.metadata,
        &evidence.cpu.vendor.metadata,
        &evidence.cpu.physical_core_count.metadata,
        &evidence.cpu.logical_core_count.metadata,
        &evidence.physical_memory.total_bytes.metadata,
        &evidence.physical_memory.available_bytes.metadata,
        &evidence.gpus.metadata,
        &evidence.storage.capacity_bytes.metadata,
        &evidence.storage.free_bytes.metadata,
        &evidence.storage.filesystem.metadata,
        &evidence.storage.media_kind.metadata,
    ];
    for gpu in &evidence.gpus.devices {
        metadata.extend([
            &gpu.name.metadata,
            &gpu.vendor.metadata,
            &gpu.dedicated_memory_bytes.metadata,
            &gpu.shared_memory_bytes.metadata,
        ]);
    }
    for acceleration in &evidence.acceleration {
        metadata.push(&acceleration.metadata);
    }

    if metadata.iter().all(|item| item.is_available()) {
        MachineProfileCompleteness::Complete
    } else {
        MachineProfileCompleteness::Partial
    }
}

fn unix_timestamp_millis() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use gixgiz_contracts::{
        AccelerationKind, ArchitectureEvidence, EvidenceAvailability, EvidenceConfidence,
        EvidenceMetadata, EvidenceSource, GpuEvidence, MachineArchitecture, StorageLocation,
        StorageMediaEvidence, StorageMediaKind, StringEvidence, U32Evidence, U64Evidence,
        UnknownReasonCode,
    };

    use super::*;

    struct FakeProvider {
        result: Result<CollectedHardwareEvidence, CoreError>,
    }

    struct SlowProvider;

    impl HardwareProvider for FakeProvider {
        fn collect(
            &self,
            context: &OperationContext,
        ) -> Result<CollectedHardwareEvidence, CoreError> {
            context.check()?;
            self.result.clone()
        }
    }

    impl HardwareProvider for SlowProvider {
        fn collect(
            &self,
            context: &OperationContext,
        ) -> Result<CollectedHardwareEvidence, CoreError> {
            let started = std::time::Instant::now();
            while started.elapsed() < std::time::Duration::from_secs(1) {
                context.check()?;
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            Ok(evidence(true))
        }
    }

    #[test]
    fn complete_and_partial_profiles_are_distinct_domain_results() {
        let complete = HardwareScanner::new(Arc::new(FakeProvider {
            result: Ok(evidence(true)),
        }));
        let partial = HardwareScanner::new(Arc::new(FakeProvider {
            result: Ok(evidence(false)),
        }));
        let context = OperationContext::generated();

        assert_eq!(
            complete
                .scan(OperationId::new(), &context)
                .expect("complete profile scans")
                .completeness,
            MachineProfileCompleteness::Complete
        );
        assert_eq!(
            partial
                .scan(OperationId::new(), &context)
                .expect("partial profile scans")
                .completeness,
            MachineProfileCompleteness::Partial
        );
    }

    #[test]
    fn cancellation_and_timeout_remain_distinct() {
        let scanner = HardwareScanner::new(Arc::new(FakeProvider {
            result: Ok(evidence(true)),
        }));
        let cancelled = OperationContext::generated();
        cancelled.cancellation().cancel();
        let timed_out = OperationContext::generated().with_timeout(std::time::Duration::ZERO);

        assert_eq!(
            scanner.scan(OperationId::new(), &cancelled),
            Err(CoreError::Cancelled)
        );
        assert_eq!(
            scanner.scan(OperationId::new(), &timed_out),
            Err(CoreError::TimedOut)
        );
    }

    #[test]
    fn provider_permission_denial_maps_without_raw_details() {
        let scanner = HardwareScanner::new(Arc::new(FakeProvider {
            result: Err(CoreError::HardwarePermissionDenied),
        }));

        assert_eq!(
            scanner.scan(OperationId::new(), &OperationContext::generated()),
            Err(CoreError::HardwarePermissionDenied)
        );
        let context = OperationContext::generated();
        let payload = CoreError::HardwarePermissionDenied.to_safe_payload(&context);
        assert_eq!(payload.code, "hardware.permission_denied");
        assert!(!payload.message.to_ascii_lowercase().contains("cim"));
    }

    #[test]
    fn provider_collection_observes_the_operation_deadline() {
        let scanner = HardwareScanner::new(Arc::new(SlowProvider));
        let context =
            OperationContext::generated().with_timeout(std::time::Duration::from_millis(5));

        assert_eq!(
            scanner.scan(OperationId::new(), &context),
            Err(CoreError::TimedOut)
        );
    }

    fn evidence(complete: bool) -> CollectedHardwareEvidence {
        let available =
            EvidenceMetadata::available(EvidenceSource::WindowsCim, EvidenceConfidence::High);
        let maybe = if complete {
            available.clone()
        } else {
            EvidenceMetadata::unavailable(
                EvidenceSource::WindowsCim,
                EvidenceAvailability::NotReliable,
                UnknownReasonCode::SourceUnreliable,
                "The provider did not report reliable video memory.",
            )
        };
        let string = |value: &str| StringEvidence {
            value: Some(value.to_owned()),
            metadata: available.clone(),
        };
        CollectedHardwareEvidence {
            operating_system: OperatingSystemEvidence {
                name: string("Windows"),
                version: string("1"),
                build: string("1"),
                architecture: ArchitectureEvidence {
                    value: Some(MachineArchitecture::X86_64),
                    metadata: available.clone(),
                },
            },
            cpu: CpuEvidence {
                name: string("Test CPU"),
                vendor: string("Test vendor"),
                physical_core_count: U32Evidence {
                    value: Some(4),
                    metadata: available.clone(),
                },
                logical_core_count: U32Evidence {
                    value: Some(8),
                    metadata: available.clone(),
                },
            },
            physical_memory: PhysicalMemoryEvidence {
                total_bytes: U64Evidence {
                    value: Some(16 * 1024 * 1024),
                    metadata: available.clone(),
                },
                available_bytes: U64Evidence {
                    value: Some(8 * 1024 * 1024),
                    metadata: available.clone(),
                },
            },
            gpus: GpuCollectionEvidence {
                devices: vec![GpuEvidence {
                    name: string("Test GPU"),
                    vendor: string("Test vendor"),
                    dedicated_memory_bytes: U64Evidence {
                        value: complete.then_some(1024),
                        metadata: maybe.clone(),
                    },
                    shared_memory_bytes: U64Evidence {
                        value: Some(1024),
                        metadata: available.clone(),
                    },
                }],
                metadata: available.clone(),
            },
            acceleration: vec![AccelerationEvidence {
                kind: AccelerationKind::DirectMl,
                supported: Some(true),
                metadata: available.clone(),
            }],
            storage: StorageEvidence {
                location: StorageLocation::ApplicationData,
                capacity_bytes: U64Evidence {
                    value: Some(1024),
                    metadata: available.clone(),
                },
                free_bytes: U64Evidence {
                    value: Some(512),
                    metadata: available.clone(),
                },
                filesystem: string("NTFS"),
                media_kind: StorageMediaEvidence {
                    value: Some(StorageMediaKind::Fixed),
                    metadata: available,
                },
            },
        }
    }
}
