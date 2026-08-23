use std::{
    env, fs,
    io::{self, Read},
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use gixgiz_contracts::{
    AccelerationEvidence, AccelerationKind, ArchitectureEvidence, CpuEvidence,
    EvidenceAvailability, EvidenceConfidence, EvidenceMetadata, EvidenceSource,
    GpuCollectionEvidence, GpuEvidence, MachineArchitecture, OperatingSystemEvidence,
    PhysicalMemoryEvidence, StorageEvidence, StorageLocation, StorageMediaEvidence,
    StorageMediaKind, StringEvidence, U32Evidence, U64Evidence, UnknownReasonCode,
};
use serde::Deserialize;

use super::{CollectedHardwareEvidence, HardwareProvider};
use crate::{CoreError, OperationContext};

const MAX_STDOUT_BYTES: usize = 256 * 1024;
const MAX_STDERR_BYTES: usize = 32 * 1024;
const MAX_TEXT_BYTES: usize = 256;
const MAX_PROCESSORS: usize = 8;
const MAX_GPUS: usize = 16;
const POLL_INTERVAL: Duration = Duration::from_millis(20);

const WINDOWS_EVIDENCE_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
function Get-GixGizCimSection([string] $ClassName, [string[]] $Properties) {
  try {
    $items = @(Get-CimInstance -ClassName $ClassName -Property $Properties -ErrorAction Stop)
    if ($items.Count -eq 0) {
      return [pscustomobject]@{ status = 'not_present'; items = @() }
    }
    return [pscustomobject]@{ status = 'available'; items = @($items | Select-Object -Property $Properties) }
  } catch {
    $denied = $_.Exception.HResult -eq -2147024891 -or $_.FullyQualifiedErrorId -like '*AccessDenied*'
    return [pscustomobject]@{ status = $(if ($denied) { 'permission_denied' } else { 'unavailable' }); items = @() }
  }
}

$os = Get-GixGizCimSection 'Win32_OperatingSystem' @('Caption', 'Version', 'BuildNumber', 'OSArchitecture', 'TotalVisibleMemorySize', 'FreePhysicalMemory')
$processors = Get-GixGizCimSection 'Win32_Processor' @('Name', 'Manufacturer', 'NumberOfCores', 'NumberOfLogicalProcessors')
$gpus = Get-GixGizCimSection 'Win32_VideoController' @('Name', 'AdapterCompatibility')
$allDisks = Get-GixGizCimSection 'Win32_LogicalDisk' @('DeviceID', 'Size', 'FreeSpace', 'FileSystem', 'DriveType')
$root = [System.IO.Path]::GetPathRoot($env:LOCALAPPDATA)
if ($allDisks.status -eq 'available' -and $root) {
  $deviceId = $root.TrimEnd([char]92)
  $selected = @($allDisks.items | Where-Object { $_.DeviceID -eq $deviceId })
  $storage = if ($selected.Count -gt 0) {
    [pscustomobject]@{ status = 'available'; items = @($selected) }
  } else {
    [pscustomobject]@{ status = 'not_present'; items = @() }
  }
} else {
  $storage = [pscustomobject]@{ status = $allDisks.status; items = @() }
}

[pscustomobject]@{
  operatingSystem = $os
  processors = $processors
  gpus = $gpus
  storage = $storage
} | ConvertTo-Json -Compress -Depth 6
"#;

/// Non-elevated Windows hardware evidence provider.
#[derive(Clone, Debug)]
pub struct WindowsHardwareProvider {
    powershell_path: PathBuf,
}

impl WindowsHardwareProvider {
    /// Resolves the fixed inbox Windows PowerShell executable.
    pub fn new() -> Result<Self, CoreError> {
        #[cfg(not(windows))]
        {
            Err(CoreError::HardwareProviderUnavailable)
        }

        #[cfg(windows)]
        {
            let system_root = env::var_os("SystemRoot")
                .map(PathBuf::from)
                .ok_or(CoreError::HardwareProviderUnavailable)?;
            let powershell_path = system_root
                .join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe");
            let canonical_root = fs::canonicalize(&system_root)
                .map_err(|_| CoreError::HardwareProviderUnavailable)?;
            let canonical_executable = fs::canonicalize(&powershell_path)
                .map_err(|_| CoreError::HardwareProviderUnavailable)?;
            if !canonical_executable.starts_with(canonical_root) {
                return Err(CoreError::HardwareProviderUnavailable);
            }
            Ok(Self {
                powershell_path: canonical_executable,
            })
        }
    }

    fn collect_raw(&self, context: &OperationContext) -> Result<RawEnvelope, CoreError> {
        context.check()?;
        let mut command = Command::new(&self.powershell_path);
        command
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                WINDOWS_EVIDENCE_SCRIPT,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = command
            .spawn()
            .map_err(|_| CoreError::HardwareProviderUnavailable)?;
        let stdout = child
            .stdout
            .take()
            .ok_or(CoreError::HardwareProviderUnavailable)?;
        let stderr = child
            .stderr
            .take()
            .ok_or(CoreError::HardwareProviderUnavailable)?;
        let stdout_reader = thread::spawn(move || drain_bounded(stdout, MAX_STDOUT_BYTES));
        let stderr_reader = thread::spawn(move || drain_bounded(stderr, MAX_STDERR_BYTES));

        let status = loop {
            if let Err(error) = context.check() {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(error);
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => thread::sleep(POLL_INTERVAL),
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                    return Err(CoreError::HardwareProviderUnavailable);
                }
            }
        };

        let stdout = join_output(stdout_reader)?;
        let stderr = join_output(stderr_reader)?;
        if stdout.truncated || stderr.truncated {
            return Err(CoreError::HardwareProviderOutputLimit);
        }
        if !status.success() {
            return Err(CoreError::HardwareProviderUnavailable);
        }

        serde_json::from_slice(&stdout.bytes).map_err(|_| CoreError::HardwareProviderInvalidData)
    }
}

impl HardwareProvider for WindowsHardwareProvider {
    fn collect(&self, context: &OperationContext) -> Result<CollectedHardwareEvidence, CoreError> {
        normalize(self.collect_raw(context)?)
    }
}

struct BoundedOutput {
    bytes: Vec<u8>,
    truncated: bool,
}

fn drain_bounded(mut reader: impl Read, limit: usize) -> io::Result<BoundedOutput> {
    let mut bytes = Vec::with_capacity(limit.min(8192));
    let mut buffer = [0_u8; 8192];
    let mut truncated = false;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let available = limit.saturating_sub(bytes.len());
        let retained = read.min(available);
        bytes.extend_from_slice(&buffer[..retained]);
        truncated |= retained < read;
    }
    Ok(BoundedOutput { bytes, truncated })
}

fn join_output(
    reader: thread::JoinHandle<io::Result<BoundedOutput>>,
) -> Result<BoundedOutput, CoreError> {
    reader
        .join()
        .map_err(|_| CoreError::HardwareProviderUnavailable)?
        .map_err(|_| CoreError::HardwareProviderUnavailable)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawEnvelope {
    operating_system: RawSection<RawOperatingSystem>,
    processors: RawSection<RawProcessor>,
    gpus: RawSection<RawGpu>,
    storage: RawSection<RawStorage>,
}

#[derive(Debug, Deserialize)]
struct RawSection<T> {
    status: String,
    items: Vec<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawOperatingSystem {
    caption: Option<String>,
    version: Option<String>,
    build_number: Option<String>,
    // WMI reports `OSArchitecture`, which PascalCase would render as
    // `OsArchitecture` and never match. Pin the exact property name.
    #[serde(rename = "OSArchitecture")]
    os_architecture: Option<String>,
    total_visible_memory_size: Option<u64>,
    free_physical_memory: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawProcessor {
    name: Option<String>,
    manufacturer: Option<String>,
    number_of_cores: Option<u32>,
    number_of_logical_processors: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawGpu {
    name: Option<String>,
    adapter_compatibility: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawStorage {
    // WMI reports `DeviceID`; PascalCase would render `DeviceId`.
    #[allow(dead_code)]
    #[serde(rename = "DeviceID")]
    device_id: Option<String>,
    size: Option<u64>,
    free_space: Option<u64>,
    file_system: Option<String>,
    drive_type: Option<u32>,
}

fn normalize(raw: RawEnvelope) -> Result<CollectedHardwareEvidence, CoreError> {
    if raw.processors.items.len() > MAX_PROCESSORS || raw.gpus.items.len() > MAX_GPUS {
        return Err(CoreError::HardwareProviderOutputLimit);
    }

    let os_meta = section_metadata(
        &raw.operating_system.status,
        raw.operating_system.items.is_empty(),
        EvidenceSource::WindowsCim,
    );
    let os = raw.operating_system.items.first();
    let processor_meta = section_metadata(
        &raw.processors.status,
        raw.processors.items.is_empty(),
        EvidenceSource::WindowsCim,
    );
    let gpu_meta = section_metadata(
        &raw.gpus.status,
        raw.gpus.items.is_empty(),
        EvidenceSource::WindowsCim,
    );
    let storage_meta = section_metadata(
        &raw.storage.status,
        raw.storage.items.is_empty(),
        EvidenceSource::WindowsCim,
    );
    let storage = raw.storage.items.first();

    Ok(CollectedHardwareEvidence {
        operating_system: OperatingSystemEvidence {
            name: string_evidence(os.and_then(|item| item.caption.as_deref()), &os_meta),
            version: string_evidence(os.and_then(|item| item.version.as_deref()), &os_meta),
            build: string_evidence(os.and_then(|item| item.build_number.as_deref()), &os_meta),
            architecture: architecture_evidence(
                os.and_then(|item| item.os_architecture.as_deref()),
                &os_meta,
            ),
        },
        cpu: CpuEvidence {
            name: consistent_string(
                &raw.processors.items,
                |item| item.name.as_deref(),
                &processor_meta,
            ),
            vendor: consistent_string(
                &raw.processors.items,
                |item| item.manufacturer.as_deref(),
                &processor_meta,
            ),
            physical_core_count: summed_u32(
                &raw.processors.items,
                |item| item.number_of_cores,
                &processor_meta,
            ),
            logical_core_count: summed_u32(
                &raw.processors.items,
                |item| item.number_of_logical_processors,
                &processor_meta,
            ),
        },
        physical_memory: PhysicalMemoryEvidence {
            total_bytes: kibibytes_evidence(
                os.and_then(|item| item.total_visible_memory_size),
                &os_meta,
            ),
            available_bytes: kibibytes_evidence(
                os.and_then(|item| item.free_physical_memory),
                &os_meta,
            ),
        },
        gpus: GpuCollectionEvidence {
            devices: if gpu_meta.is_available() {
                raw.gpus
                    .items
                    .iter()
                    .map(|gpu| GpuEvidence {
                        name: string_evidence(gpu.name.as_deref(), &gpu_meta),
                        vendor: string_evidence(gpu.adapter_compatibility.as_deref(), &gpu_meta),
                        dedicated_memory_bytes: unreliable_memory(),
                        shared_memory_bytes: unreliable_memory(),
                    })
                    .collect()
            } else {
                Vec::new()
            },
            metadata: gpu_meta,
        },
        acceleration: [
            AccelerationKind::DirectMl,
            AccelerationKind::Cuda,
            AccelerationKind::Rocm,
        ]
        .into_iter()
        .map(|kind| AccelerationEvidence {
            kind,
            supported: None,
            metadata: EvidenceMetadata::unavailable(
                EvidenceSource::WindowsCim,
                EvidenceAvailability::NotReliable,
                UnknownReasonCode::ProviderUnsupported,
                "Display-adapter names do not prove acceleration support.",
            ),
        })
        .collect(),
        storage: StorageEvidence {
            location: StorageLocation::ApplicationData,
            capacity_bytes: u64_evidence(storage.and_then(|item| item.size), &storage_meta),
            free_bytes: u64_evidence(storage.and_then(|item| item.free_space), &storage_meta),
            filesystem: string_evidence(
                storage.and_then(|item| item.file_system.as_deref()),
                &storage_meta,
            ),
            media_kind: media_evidence(storage.and_then(|item| item.drive_type), &storage_meta),
        },
    })
}

fn section_metadata(status: &str, empty: bool, source: EvidenceSource) -> EvidenceMetadata {
    match status {
        "available" if !empty => EvidenceMetadata::available(source, EvidenceConfidence::High),
        "available" | "not_present" => EvidenceMetadata::unavailable(
            source,
            EvidenceAvailability::NotPresent,
            UnknownReasonCode::DeviceAbsent,
            "Windows did not report a matching device.",
        ),
        "permission_denied" => EvidenceMetadata::unavailable(
            source,
            EvidenceAvailability::PermissionDenied,
            UnknownReasonCode::AccessDenied,
            "The current user could not access this evidence.",
        ),
        _ => EvidenceMetadata::unavailable(
            source,
            EvidenceAvailability::Unknown,
            UnknownReasonCode::NotReported,
            "Windows did not report this evidence.",
        ),
    }
}

fn missing_metadata(base: &EvidenceMetadata) -> EvidenceMetadata {
    if base.is_available() {
        EvidenceMetadata::unavailable(
            base.source,
            EvidenceAvailability::Unknown,
            UnknownReasonCode::NotReported,
            "Windows did not report this field.",
        )
    } else {
        base.clone()
    }
}

fn bounded_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= MAX_TEXT_BYTES)
        .filter(|value| !value.chars().any(char::is_control))
        .map(str::to_owned)
}

fn string_evidence(value: Option<&str>, base: &EvidenceMetadata) -> StringEvidence {
    let value = if base.is_available() {
        bounded_text(value)
    } else {
        None
    };
    StringEvidence {
        metadata: if value.is_some() {
            base.clone()
        } else {
            missing_metadata(base)
        },
        value,
    }
}

fn u64_evidence(value: Option<u64>, base: &EvidenceMetadata) -> U64Evidence {
    let value = if base.is_available() { value } else { None };
    U64Evidence {
        metadata: if value.is_some() {
            base.clone()
        } else {
            missing_metadata(base)
        },
        value,
    }
}

fn kibibytes_evidence(value: Option<u64>, base: &EvidenceMetadata) -> U64Evidence {
    u64_evidence(value.and_then(|value| value.checked_mul(1024)), base)
}

fn consistent_string<T>(
    items: &[T],
    value: impl Fn(&T) -> Option<&str>,
    base: &EvidenceMetadata,
) -> StringEvidence {
    if !base.is_available() {
        return StringEvidence {
            value: None,
            metadata: base.clone(),
        };
    }
    let values: Vec<String> = items
        .iter()
        .filter_map(|item| bounded_text(value(item)))
        .collect();
    let first = (values.len() == items.len())
        .then(|| values.first().cloned())
        .flatten();
    let consistent = first.filter(|first| values.iter().all(|value| value == first));
    StringEvidence {
        metadata: if consistent.is_some() {
            base.clone()
        } else {
            missing_metadata(base)
        },
        value: consistent,
    }
}

fn summed_u32<T>(
    items: &[T],
    value: impl Fn(&T) -> Option<u32>,
    base: &EvidenceMetadata,
) -> U32Evidence {
    if !base.is_available() {
        return U32Evidence {
            value: None,
            metadata: base.clone(),
        };
    }
    let total = items
        .iter()
        .try_fold(0_u32, |total, item| total.checked_add(value(item)?));
    U32Evidence {
        metadata: if total.is_some() {
            base.clone()
        } else {
            missing_metadata(base)
        },
        value: total,
    }
}

fn architecture_evidence(value: Option<&str>, base: &EvidenceMetadata) -> ArchitectureEvidence {
    let value = if base.is_available() {
        bounded_text(value)
    } else {
        None
    }
    .and_then(|value| {
        let lower = value.to_ascii_lowercase();
        if lower.contains("arm") && lower.contains("64") {
            Some(MachineArchitecture::Arm64)
        } else if lower.contains("64") {
            Some(MachineArchitecture::X86_64)
        } else if lower.contains("32") || lower.contains("x86") {
            Some(MachineArchitecture::X86)
        } else {
            None
        }
    });
    ArchitectureEvidence {
        metadata: if value.is_some() {
            base.clone()
        } else {
            missing_metadata(base)
        },
        value,
    }
}

fn unreliable_memory() -> U64Evidence {
    U64Evidence {
        value: None,
        metadata: EvidenceMetadata::unavailable(
            EvidenceSource::WindowsCim,
            EvidenceAvailability::NotReliable,
            UnknownReasonCode::SourceUnreliable,
            "Windows display-adapter memory is not reliable enough for readiness decisions.",
        ),
    }
}

fn media_evidence(value: Option<u32>, base: &EvidenceMetadata) -> StorageMediaEvidence {
    let value = match value.filter(|_| base.is_available()) {
        Some(2) => Some(StorageMediaKind::Removable),
        Some(3) => Some(StorageMediaKind::Fixed),
        Some(4) => Some(StorageMediaKind::Network),
        Some(5) => Some(StorageMediaKind::Optical),
        Some(6) => Some(StorageMediaKind::RamDisk),
        Some(_) => None,
        None => None,
    };
    StorageMediaEvidence {
        metadata: if value.is_some() {
            base.clone()
        } else {
            missing_metadata(base)
        },
        value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
      "operatingSystem":{"status":"available","items":[{"Caption":"Windows 11 Pro","Version":"10.0","BuildNumber":"26100","OSArchitecture":"64-bit","TotalVisibleMemorySize":16777216,"FreePhysicalMemory":8388608}]},
      "processors":{"status":"available","items":[{"Name":"Example CPU","Manufacturer":"Example","NumberOfCores":4,"NumberOfLogicalProcessors":8}]},
      "gpus":{"status":"available","items":[{"Name":"GPU A","AdapterCompatibility":"Vendor A","AdapterRAM":4294967295},{"Name":"GPU B","AdapterCompatibility":"Vendor B","AdapterRAM":2147483648}]},
      "storage":{"status":"available","items":[{"DeviceID":"C:","Size":1000,"FreeSpace":5,"FileSystem":"NTFS","DriveType":3}]}
    }"#;

    #[test]
    fn normalizes_physical_memory_multiple_gpus_and_low_storage() {
        let raw: RawEnvelope = serde_json::from_str(FIXTURE).expect("fixture parses");
        let evidence = normalize(raw).expect("fixture normalizes");

        assert_eq!(
            evidence.physical_memory.total_bytes.value,
            Some(17_179_869_184)
        );
        assert_eq!(
            evidence.physical_memory.available_bytes.value,
            Some(8_589_934_592)
        );
        // Architecture is a hard requirement for planning: a property-name
        // mismatch here silently blocks every recommendation.
        assert_eq!(
            evidence.operating_system.architecture.value,
            Some(MachineArchitecture::X86_64)
        );
        assert_eq!(
            evidence.operating_system.architecture.metadata.availability,
            EvidenceAvailability::Available
        );
        assert_eq!(evidence.gpus.devices.len(), 2);
        assert_eq!(evidence.storage.free_bytes.value, Some(5));
        assert_eq!(
            evidence.gpus.devices[0]
                .dedicated_memory_bytes
                .metadata
                .availability,
            EvidenceAvailability::NotReliable
        );
        assert!(
            evidence
                .acceleration
                .iter()
                .all(|item| item.supported.is_none())
        );
    }

    #[test]
    fn missing_gpu_and_permission_denial_remain_partial_evidence() {
        let fixture = FIXTURE
            .replace(
                "\"status\":\"available\",\"items\":[{\"Name\":\"GPU A\"",
                "\"status\":\"not_present\",\"items\":[{\"Name\":\"GPU A\"",
            )
            .replace(
                "\"processors\":{\"status\":\"available\"",
                "\"processors\":{\"status\":\"permission_denied\"",
            );
        let raw: RawEnvelope = serde_json::from_str(&fixture).expect("fixture parses");
        let evidence = normalize(raw).expect("fixture normalizes");

        assert_eq!(
            evidence.cpu.name.metadata.availability,
            EvidenceAvailability::PermissionDenied
        );
        assert_eq!(evidence.cpu.name.value, None);
        assert_eq!(
            evidence.gpus.metadata.availability,
            EvidenceAvailability::NotPresent
        );
        assert!(evidence.gpus.devices.is_empty());
    }

    #[test]
    fn inaccessible_storage_is_unknown_and_removable_storage_is_categorical() {
        let denied_fixture = FIXTURE.replace(
            "\"storage\":{\"status\":\"available\"",
            "\"storage\":{\"status\":\"permission_denied\"",
        );
        let denied =
            normalize(serde_json::from_str(&denied_fixture).expect("denied fixture parses"))
                .expect("denied fixture normalizes");
        assert_eq!(denied.storage.capacity_bytes.value, None);
        assert_eq!(
            denied.storage.capacity_bytes.metadata.availability,
            EvidenceAvailability::PermissionDenied
        );

        let removable_fixture = FIXTURE.replace("\"DriveType\":3", "\"DriveType\":2");
        let removable =
            normalize(serde_json::from_str(&removable_fixture).expect("removable fixture parses"))
                .expect("removable fixture normalizes");
        assert_eq!(
            removable.storage.media_kind.value,
            Some(StorageMediaKind::Removable)
        );
    }

    #[test]
    fn rejects_unbounded_device_collections_and_invalid_json() {
        let mut raw: RawEnvelope = serde_json::from_str(FIXTURE).expect("fixture parses");
        for _ in 0..MAX_GPUS {
            raw.gpus.items.push(RawGpu {
                name: Some("extra".to_owned()),
                adapter_compatibility: None,
            });
        }
        assert_eq!(normalize(raw), Err(CoreError::HardwareProviderOutputLimit));
        assert!(serde_json::from_str::<RawEnvelope>("not-json").is_err());
    }

    #[test]
    fn bounded_reader_discards_excess_without_growing() {
        let output = drain_bounded(&b"0123456789"[..], 4).expect("read succeeds");
        assert_eq!(output.bytes, b"0123");
        assert!(output.truncated);
    }

    #[test]
    fn provider_script_excludes_stable_device_identifiers() {
        let script = WINDOWS_EVIDENCE_SCRIPT.to_ascii_lowercase();
        for forbidden in [
            "serialnumber",
            "uuid",
            "macaddress",
            "pnpdeviceid",
            "machineguid",
            "totalvirtualmemorysize",
        ] {
            assert!(!script.contains(forbidden), "script contains {forbidden}");
        }
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "manual non-elevated Windows evidence smoke test"]
    fn manual_windows_scan_returns_anonymized_evidence() {
        let provider = WindowsHardwareProvider::new().expect("inbox PowerShell exists");
        let evidence = provider
            .collect(&OperationContext::generated().with_timeout(Duration::from_secs(10)))
            .expect("evidence collection succeeds");
        assert_eq!(evidence.storage.location, StorageLocation::ApplicationData);
    }
}
