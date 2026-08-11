# Ollama runtime adapter development guide

## Ownership and dependency boundary

Task 09 registers one concrete v0.1 provider at compile time while keeping all
cross-process and core behavior provider-neutral:

```text
Flutter -> CoreClient -> authenticated sidecar -> gixgiz-core -> gixgiz-runtime
                                             composition |          ^
                                                         v          |
                                                gixgiz-runtime-ollama
```

`gixgiz-runtime` owns traits, cancellation/deadline conventions, normalized
errors, and deterministic fakes. `gixgiz-runtime-ollama` alone owns Ollama
executable names, environment variables, paths, API addresses and routes,
payloads, version parsing, process rules, and model tags. Flutter never calls a
provider, starts a process, or opens SQLite. The sidecar exposes typed runtime
intentions and is not an Ollama proxy or public local gateway.

## Dependency choices

Rust's standard library does not provide an asynchronous HTTP/1 client, JSON
serialization, semantic-version parser, or cancellation-aware async process
runtime. The adapter therefore reuses the workspace's Tokio, Serde/Serde JSON,
Thiserror, and Tracing dependencies, and adds narrowly featured `hyper`,
`hyper-util`, `http-body-util`, `bytes`, and `semver` dependencies. Hyper is used
directly with only client, HTTP/1, and Tokio integration so the adapter controls
the exact socket address and implements no proxy, redirect, cookie, TLS, or DNS
fallback. Semver parses provider evidence without inventing string ordering.

These crates are mature Rust ecosystem components, support Windows MSVC, and
use permissive MIT or MIT/Apache-2.0 licensing. They add Rust code only: no new
native DLL, web framework, ORM, provider SDK, code generator, or unsafe adapter
code is introduced. The versions are workspace-managed and `Cargo.lock` is
committed. A broader HTTP client was rejected because its default URL, proxy,
TLS, and redirect surface is unnecessary for one fixed loopback provider.

## Detection and normalized state

Detection is non-elevated and read-only. On Windows automatic discovery checks
only the documented per-user install location beneath `%LOCALAPPDATA%`; it does
not execute a same-name binary found on `PATH`. A caller-approved explicit path
is available only inside the adapter composition/test boundary. A candidate
must exist as a file, have the expected filename, remain under the documented
installation root when automatically discovered, and canonicalize successfully
before any structured process invocation. Before running either `--version` or
`serve`, the adapter invokes the canonical system Windows PowerShell directly
and requires a valid Authenticode status with the normalized publisher name
`Ollama Inc.`. The verifier has a three-second deadline and 256-byte output
limit. An unsigned, wrong-publisher, unavailable, or malformed verification
result fails closed without running the provider executable.

The adapter combines executable, endpoint, version, health, and owned-child
evidence into `NotInstalled`, `InstalledStopped`, `Ready`, `Degraded`, or
`Incompatible`. `Starting`, `Updating`, and `Failed` remain explicit normalized
states for operation and forward-compatible reporting. Missing or uncertain
evidence is never promoted to `Ready`.

## Endpoint and version policy

Provider HTTP uses direct TCP without a proxy, redirect, DNS-selected remote
host, cookie, or TLS fallback. Non-loopback, wildcard, and URL-shaped endpoint
values produce a safe degraded state and are never contacted. Raw provider
bodies, command output, endpoint values, executable paths, and model inventories
are not logged or returned as boundary errors.

The v0.1 support policy was reviewed on 2026-08-11 against these official
sources:

- [Ollama API introduction](https://docs.ollama.com/api/introduction), which
  describes the API as expected to remain stable and backward-compatible;
- [`GET /api/version`](https://docs.ollama.com/api-reference/get-version), whose documented
  example reports `0.12.6`;
- [`GET /api/tags`](https://docs.ollama.com/api/tags), which defines the model
  inventory resource used by the adapter; and
- [Ollama v0.32.5](https://github.com/ollama/ollama/releases/tag/v0.32.5), the
  latest stable release reviewed for this policy, published 2026-07-27.

GixGiz therefore supports stable Ollama versions from `0.12.6` through
`0.32.5`, inclusive. A stable version below `0.12.6` is `Incompatible`.
Prerelease versions and stable versions newer than `0.32.5` remain
`Untested`/`Degraded` until the policy evidence is updated. Invalid version
payloads are degraded and never promoted to ready.

| Value | Exact v0.1 implementation |
|---|---|
| Automatic executable | `%LOCALAPPDATA%\Programs\Ollama\ollama.exe`; no `PATH` lookup |
| Default endpoint | Direct HTTP/1.1 to `127.0.0.1:11434` |
| Endpoint override | `OLLAMA_HOST`; literal loopback IPv4/IPv6 or exact `localhost`, optional nonzero port, no credentials, path, or query |
| Version and health | `GET /api/version` |
| Model inventory | `GET /api/tags` |
| Executable version | Structured `ollama.exe --version`; no shell |
| Managed child start | Structured `ollama.exe serve` with validated `OLLAMA_HOST`; no shell |
| Executable trust | Bounded `Get-AuthenticodeSignature` through canonical system Windows PowerShell; valid signature and normalized `Ollama Inc.` publisher required |
| Adapter deadlines | HTTP 3 seconds, version command 3 seconds, stop 5 seconds, post-commit observation 7 seconds |
| Host lifecycle deadline | 30 seconds |
| Output limits | Version body 4 KiB, tags body 1 MiB, command output 4 KiB |
| Inventory limits | Parse at most 1,024 provider rows, return at most 256 adapter rows, expose at most 100 core rows |

## Ownership, consent, and lifecycle

Detection never writes ownership. A present runtime without a policy record is
reported as `External`. The Rust-owned schema stores ownership, reuse consent,
and management consent as separate bounded fields. Approving reuse preserves
external ownership and does not grant management.

Read-only model listing is available only after core policy authorizes reuse.
Start, stop, and restart require both `GixGizManaged` or `Bundled` ownership and
explicit management approval. The Task 09 UI has no ownership-transfer or
management-approval workflow, so a discovered external installation cannot be
controlled through this slice. At the adapter boundary, lifecycle control is
limited to the exact child handle the adapter created with a structured
`ollama serve` invocation; it never searches for, kills, or reconfigures an
external process. Stop and restart refuse a process the adapter does not own.

Lifecycle requests carry correlation/request IDs, a deadline, and a
cancellation token. The sidecar allows one active lifecycle operation, retains
at most eight operation records for bounded replay, emits ordered SSE events,
and has a fixed 30-second operation deadline. A failed or cancelled start
attempts to stop only its own child and reports the terminal result honestly.
Stopping has one bounded non-interruptible commit point once termination of the
exact retained child begins. A restart cancelled after that stop preserves the
cancelled/timed-out terminal state; the host performs a separate three-second
status refresh and attaches the authoritative retained state to the event. No
automatic retry loop is used.

## Model inventory

The adapter parses a bounded provider model list and maps only allowlisted known
tags to canonical catalogue IDs. Unknown tags remain explicit external models;
a tag alone does not prove integrity or inference readiness. Core limits a
request to 100 returned items. Inventory is loaded only after an explicit UI
action, is not persisted, and is not included in routine logs or status text.

| Provider tag | GixGiz catalogue ID |
|---|---|
| `qwen2.5:0.5b-instruct` | `qwen2.5.0.5b-instruct` |
| `qwen2.5:1.5b-instruct` | `qwen2.5.1.5b-instruct` |
| `qwen2.5:7b-instruct` | `qwen2.5.7b-instruct` |
| `qwen2.5-coder:0.5b-instruct` | `qwen2.5-coder.0.5b-instruct` |
| `qwen2.5-coder:1.5b-instruct` | `qwen2.5-coder.1.5b-instruct` |
| `qwen2.5-coder:7b-instruct` | `qwen2.5-coder.7b-instruct` |

Task 09 does not pull, download, delete, update, or verify a model through
inference. Those workflows remain separate tasks.

## Deterministic and real-provider tests

Normal unit, contract, host, persistence, and Flutter tests use fake process,
HTTP, provider, and policy boundaries. They verify the exact lower and upper
version bounds, older, prerelease, and future versions, both required HTTP
resources, lifecycle behavior, and all compiled model-tag mappings. They
require no provider installation, network access, elevation, model data, or
user application-data directory.

The real-provider smoke test is ignored by default and is deliberately
read-only. On an explicitly prepared Windows machine with a local Ollama
instance already running on an approved loopback endpoint:

```powershell
$env:GIXGIZ_RUN_REAL_OLLAMA_SMOKE = '1'
cargo test -p gixgiz-runtime-ollama --test real_ollama -- --ignored --nocapture
Remove-Item Env:GIXGIZ_RUN_REAL_OLLAMA_SMOKE
```

The test reports the provider version, verifies that normalized state agrees
with the production compatibility policy and loopback-safe endpoint, and
requests at most 16 model summaries when policy permits read-only inventory. It
does not install, start, stop, restart, update, pull, delete, chat, or infer.
Record the provider version, endpoint class, ownership scenario, and result in
manual evidence when running it. Passing deterministic fakes proves adapter
logic; it is not evidence that a real provider was exercised.

## Troubleshooting

- `NotInstalled`: no validated executable and no healthy approved endpoint was
  found. Task 09 offers requirements/refresh only; installation is deferred.
- `InstalledStopped`: a compatible executable exists but the endpoint is not
  ready. External lifecycle control remains unavailable.
- `Degraded`: endpoint safety, version, payload, or capability evidence is
  incomplete. Restore a loopback-only configuration and retry status.
- `Incompatible`: verified policy rejects the version or required capability.
  Task 09 performs no automatic update.
- `PermissionDenied` or consent-required: review the explicit reuse decision;
  reuse approval still does not transfer ownership.
- `Busy`, `TimedOut`, or `Cancelled`: wait for the active bounded operation or
  retry after checking the authoritative status. Cancellation is not failure.

Do not work around a safe failure by exposing a provider remotely, running the
desktop/core elevated, editing SQLite, or invoking an external process from
Flutter.

## Known limitations

- The supported range is backed by the official compatibility statement,
  documented `0.12.6` API example, current tags contract, and reviewed `0.32.5`
  release. The opt-in smoke has not yet verified a real provider version in the
  current task environment.
- Discovery validates the canonical path, expected executable name, valid
  Authenticode status, normalized publisher name, semantic version, loopback
  endpoint, and implemented capabilities. It does not pin a publisher
  certificate/thumbprint or an installer-manifest hash; packaging must add
  exact artifact identity and revalidation policy before managed installation.
- Task 09 has no ownership-transfer or management-approval workflow. External
  installations therefore remain read-only even after reuse approval.
