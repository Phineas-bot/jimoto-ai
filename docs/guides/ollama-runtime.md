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
errors, setup progress, and deterministic fakes. `gixgiz-runtime-ollama` alone owns Ollama
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

Task 10 also uses `fs4` 1.1.0 with default features disabled for one
cross-platform, non-mutating available-space query. The standard library has no
equivalent filesystem-capacity API. The query runs on a blocking worker and
does not enable async runtimes, file locking, native DLLs, or provider SDKs.

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
  inventory resource used by the adapter;
- [`POST /api/pull`](https://docs.ollama.com/api/pull) and
  [streaming](https://docs.ollama.com/api/streaming), which define the bounded
  NDJSON acquisition stream;
- [`POST /api/generate`](https://docs.ollama.com/api/generate), which defines
  the fixed non-streaming readiness request;
- [model storage](https://docs.ollama.com/faq#where-are-models-stored), which
  documents the Windows default and `OLLAMA_MODELS` override; and
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
| Model acquisition | `POST /api/pull`; allowlisted exact tag, `insecure: false`, streamed NDJSON |
| Readiness gate | `POST /api/generate`; fixed prompt, non-streaming, thinking disabled, output discarded |
| Executable version | Structured `ollama.exe --version`; no shell |
| Managed child start | Structured `ollama.exe serve` with validated `OLLAMA_HOST`; no shell |
| Executable trust | Bounded `Get-AuthenticodeSignature` through canonical system Windows PowerShell; valid signature and normalized `Ollama Inc.` publisher required |
| Adapter deadlines | Read-only HTTP/connect 3 seconds, pull idle 60 seconds, readiness idle 30 seconds, version command 3 seconds, stop 5 seconds, post-commit observation 7 seconds |
| Host lifecycle deadline | 30 seconds |
| Output limits | Version body 4 KiB, tags body 1 MiB, pull stream 8 MiB total/16 KiB per line/100,000 events, readiness body 64 KiB, command output 4 KiB |
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

## Task 10 model preparation and storage

Setup maps only the six canonical catalogue IDs in the table above to exact
provider tags. A caller cannot supply an arbitrary tag, route, registry URL, or
destination path. The adapter returns a generic provider-managed destination
label; neither `%USERPROFILE%` nor another private local path crosses the
runtime or Flutter boundary.

When the adapter is composed, it captures Ollama's provider-managed model root
from the visible `OLLAMA_MODELS` value or documented current-user default.
Preflight canonicalizes and pins a drive-qualified path anchor before querying
caller-available capacity. Relative, drive-relative, UNC, device, changed, or
unavailable roots fail closed. A missing leaf directory is intentionally
allowed for the first pull, so the adapter walks upward only on `NotFound`; an
inaccessible path or a file fails without climbing to a broader parent. Once
the exact root is observed, its disappearance also fails closed. Core supplies
the conservative required bytes and safety margin from
the persisted recommendation. Overflow is invalid input; known insufficient
space and destination disappearance are explicit preflight results. The
adapter creates no directory and performs no staging, relocation, quarantine,
or cleanup itself. Ollama exposes no API that proves the running process's
effective storage root, so this is explicitly environment/default-root
evidence rather than a provider attestation.

The acquisition call first reuses an exact local registration when present,
then permits at most one active pull per adapter instance. It sends
`POST /api/pull` only for the allowlisted tag with insecure registry access
disabled. The documented NDJSON stream is parsed incrementally. Provider status
and error strings are validated, classified, and discarded; they are never
logged or exposed as user text. Normalized progress contains only phase,
completed bytes, total bytes, and optional basis points. The recommended
channel capacity is 16. Intermediate updates may be coalesced or dropped when
the consumer is behind, while the persisted job snapshot and method result
remain authoritative. A successful pull is not enough: the adapter repeats
version and `GET /api/tags` inspection and requires the exact local tag before
returning success.

Cancellation drops the active loopback request through the shared runtime
cancellation token. Ollama's documented pull API does not provide a separate
server-side cancellation/rollback acknowledgement, so provider-managed partial
or even completed data may remain. The adapter never guesses that it was
removed and never deletes it. Core recovery must inspect exact registration and
report retained or uncertain effects before retrying. Retries are idempotent at
the adapter boundary because exact registration is checked again before pull.

Ollama reports a digest and a verification phase, but the current catalogue
does not contain an independently trusted expected artifact digest. Registered
models are therefore classified only as `ProviderReported`, never `Verified`.
Malformed provider integrity evidence fails closed. Independent checksum
mismatch quarantine cannot be claimed until versioned trusted digest metadata
and a provider-safe non-destructive quarantine mechanism exist.

## Bounded readiness inference

Readiness requires compatible runtime health and exact local registration, then
sends one fixed `POST /api/generate` request. Streaming and thinking are
disabled, `num_predict` is 8, `num_ctx` is 512, temperature is zero, and
`keep_alive` is zero. The caller supplies the overall deadline; setup uses 180
seconds. The adapter additionally bounds connect time to three seconds, idle
time to 30 seconds, and the full response to 64 KiB.

Only a successful terminal response with non-empty generated content passes.
The prompt and generated content are not returned, persisted, traced, or
logged. This is a setup readiness gate, not a chat or general inference API.

## Deterministic and real-provider tests

Normal unit, contract, host, persistence, and Flutter tests use fake process,
HTTP, provider, and policy boundaries. They verify the exact lower and upper
version bounds, older, prerelease, and future versions, both required HTTP
resources, lifecycle behavior, and all compiled model-tag mappings. They
require no provider installation, network access, elevation, model data, or
user application-data directory.

The first real-provider smoke test is ignored by default and is deliberately
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

The separate Task 10 smoke is mutating and requires two exact gates. It may
download and always retains the compact allowlisted model; it never deletes or
relocates provider data. Ensure at least the conservative 3 GiB preflight margin
is available and that the local endpoint is loopback-only:

```powershell
$env:GIXGIZ_RUN_REAL_OLLAMA_SETUP_SMOKE = '1'
$env:GIXGIZ_REAL_OLLAMA_ACQUISITION_APPROVED = 'qwen2.5.0.5b-instruct'
cargo test -p gixgiz-runtime-ollama --test real_model_setup -- --ignored --nocapture
Remove-Item Env:GIXGIZ_RUN_REAL_OLLAMA_SETUP_SMOKE
Remove-Item Env:GIXGIZ_REAL_OLLAMA_ACQUISITION_APPROVED
```

The smoke allows up to six hours for acquisition and 180 seconds for the fixed
readiness inference. Record provider version, terminal acquisition status,
measured bytes, normalized update count, retained model effect, and result. A
passing result is environmental evidence only for the exact machine and
provider version used.

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
- Model-storage attention: make the provider-managed destination available or
  free the conservative required bytes plus margin, then rerun preflight.
- Model-registration or readiness failure: refresh runtime health and inspect
  exact registration before retrying. A provider pull response alone is not
  readiness evidence.

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
- Task 10 may add an approved model to a reused external runtime's
  provider-managed store, but it does not update, reconfigure, relocate, or
  remove that runtime or its data. This effect must be explicit in the durable
  setup approval record.
- The current provider source supplies only provider-reported integrity. No
  independent digest expectation or safe quarantine/delete operation is
  available, so checksum-mismatch quarantine remains unverified rather than
  being represented as complete.
