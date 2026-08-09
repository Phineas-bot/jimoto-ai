# Windows hardware evidence scan

Task 07 adds the first end-to-end product operation across Flutter, the authenticated sidecar, Rust core orchestration, and a Windows evidence provider. The operation produces evidence only. It does not score capability, recommend a model, install software, call a runtime, or change Windows configuration.

## Ownership and flow

```text
Foundation UI -> CoreClient -> authenticated hardware-scan routes
                                      |
                    gixgiz-desktop-host operation registry
                                      |
                    gixgiz-core HardwareScanner
                                      |
                    WindowsHardwareProvider
```

- `gixgiz-contracts` owns the versioned `MachineProfile`, field evidence metadata, scan events, and safe errors.
- `gixgiz-core` owns scan orchestration, normalization, completeness policy, cancellation, timeout, and the Windows provider interface.
- `gixgiz-desktop-host` runs blocking collection off the async executor and adapts ordered events to the existing authenticated SSE boundary.
- Flutter starts, observes, and cancels the operation through `CoreClient`; it renders typed evidence and never invokes PowerShell or CIM directly.

Profiles are process-local in Task 07. They are not written to SQLite, configuration, logs, telemetry, or a remote service.

## Evidence collected

The provider starts the canonical inbox `System32\WindowsPowerShell\v1.0\powershell.exe` with a fixed argument list, no profile, no interactive input, and a repository-owned script constant. The script performs non-elevated CIM reads for these allowlisted fields:

| Area | Source fields | Normalization |
|---|---|---|
| Operating system | `Caption`, `Version`, `BuildNumber`, `OSArchitecture` | Bounded text and normalized architecture. |
| CPU | `Name`, `Manufacturer`, `NumberOfCores`, `NumberOfLogicalProcessors` | Consistent package identity and checked sums. |
| Physical memory | `TotalVisibleMemorySize`, `FreePhysicalMemory` | KiB converted to bytes; virtual-memory totals are not queried. |
| Display adapters | `Name`, `AdapterCompatibility` | Bounded enumeration; unreliable WMI memory values are not requested. |
| Selected storage | `Size`, `FreeSpace`, `FileSystem`, `DriveType` for the `%LOCALAPPDATA%` drive | Capacity, free space, filesystem, and categorical media kind; no raw user path crosses the boundary. |

DirectML, CUDA, and ROCm support remain unknown because adapter names do not prove runtime support. Dedicated and shared GPU memory also remain unknown until a reliable provider exists. These are valid partial-profile results rather than generic failures.

## Unknown and failure behavior

Each field carries a source, availability, confidence, optional stable reason code, and bounded safe explanation. Missing, inaccessible, unsupported, or unreliable fields use explicit non-available states. Partial evidence is returned to the UI. A provider-process failure, invalid JSON, excessive output, timeout, and user cancellation have distinct typed terminal states.

The provider:

- accepts no user-controlled executable, script, class name, property, filter, or shell fragment;
- captures at most 256 KiB stdout and 32 KiB stderr while continuing to drain pipes;
- never returns or logs raw stderr;
- polls cancellation and the operation deadline, then kills and waits for the child process;
- permits only one active scan and retains at most eight completed scan records in memory.

## Privacy

The query intentionally excludes serial numbers, UUIDs, machine GUIDs, MAC addresses, PNP device IDs, disk volume identifiers, user names, file names, directory contents, and network configuration. The selected storage location crosses the boundary only as `application_data`; the raw `%LOCALAPPDATA%` path does not.

The contract schema includes a regression assertion for common stable-identifier field names. Fixture data is synthetic and contains no real machine data.

## Validation

Run the normal Task 06 checks. Targeted Rust coverage is included in:

```powershell
cargo test -p gixgiz-core hardware --all-features
cargo test -p gixgiz-desktop-host hardware_scans --all-features
```

An ignored Windows-only smoke test is available for an explicit non-elevated manual check:

```powershell
cargo test -p gixgiz-core manual_windows_scan_returns_anonymized_evidence -- --ignored
```

The smoke test asserts only categorical storage location and does not print machine identity. Representative hardware combinations beyond the synthetic fixtures require separate manual verification on supported Windows machines.
