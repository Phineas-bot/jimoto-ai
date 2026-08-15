# Flutter-Rust transport development

Task 04 connects the Flutter desktop shell to the supervised Rust core without moving platform policy into Dart. ADR 0003 remains authoritative for the architecture; this guide records the implemented development workflow and current security boundary.

## Ownership

```text
Flutter widgets -> CoreClient -> SidecarCoreClient -> authenticated loopback API
                                                       |
gixgiz-desktop-host -> gixgiz-core -> gixgiz-contracts
```

- `gixgiz-contracts` owns all serialized handshake, health, error, event, cancellation, and shutdown types.
- `gixgiz-core` owns lifecycle and authoritative readiness policy. It does not depend on HTTP or Flutter.
- `gixgiz-core` also owns runtime reuse and management authorization; the host only adapts authorized provider-neutral intentions.
- `gixgiz-desktop-host` adapts core services to the internal authenticated loopback API and owns process bootstrap/supervision.
- Flutter accesses the core only through `CoreClient`. `SidecarCoreClient` owns production launch and transport behavior; tests can inject fake connectors and sessions.

The internal routes are not a public API and make no compatibility promise to browsers, third-party tools, Packs, MCP clients, or OpenAI-compatible clients.

## Bootstrap and lifetime

1. Flutter resolves `gixgiz-core.exe` next to `GixGiz.exe` and starts that exact path with an empty argument list and `runInShell: false`.
2. Flutter generates 32 random bytes with `Random.secure`, hex-encodes them as the per-launch bearer token, and writes one bounded JSON record to the child stdin pipe.
3. The bootstrap record includes the protocol range, desktop process ID, and an opaque supervision nonce. The token is never placed in process arguments, a URL, configuration, SQLite, or normal diagnostics.
4. Rust reads at most 8 KiB within five seconds, validates the record and token shape, binds `127.0.0.1:0`, and returns one public JSON line containing only the assigned port, instance ID, and supported versions.
5. Flutter keeps stdin open. EOF tells the sidecar that its supervisor disappeared; the sidecar begins graceful shutdown. Normal desktop exit first sends the authenticated shutdown command, closes the pipe, waits up to three seconds, then terminates the child if necessary.

The token lives only for the core process lifetime. A restarted core requires a new token and handshake.

## Internal routes

All routes require `Authorization: Bearer <per-launch-token>`, correlation and request headers, and a successful handshake before normal commands.

| Route | Purpose |
|---|---|
| `POST /internal/v1/handshake` | Negotiate protocol/capabilities and return real core identity/readiness. |
| `POST /internal/v1/health` | Return authoritative version and readiness state. |
| `POST /internal/v1/test-operations` | Start the deterministic Task 04 event operation. |
| `GET /internal/v1/test-operations/{id}/events` | Stream ordered authenticated server-sent events. |
| `POST /internal/v1/test-operations/{id}/cancel` | Request explicit cancellation. |
| `POST /internal/v1/hardware-scans` | Start one bounded Rust-owned hardware evidence scan. |
| `POST /internal/v1/recommendations` | Generate one deterministic report from supplied typed evidence and preferences. |
| `GET /internal/v1/hardware-scans/{id}/events` | Stream ordered typed scan events and the terminal machine profile. |
| `POST /internal/v1/hardware-scans/{id}/cancel` | Propagate cancellation to the Windows evidence process. |
| `POST /internal/v1/runtime/status` | Return normalized runtime evidence plus authoritative ownership and consent policy. |
| `POST /internal/v1/runtime/consent` | Record an explicit reuse decision without transferring management ownership. |
| `POST /internal/v1/runtime/models` | Return a bounded normalized inventory after core policy authorizes read-only reuse. |
| `POST /internal/v1/runtime/operations` | Start one authorized provider-neutral lifecycle operation. |
| `GET /internal/v1/runtime/operations/{id}/events` | Stream ordered lifecycle events with an explicit terminal state. |
| `POST /internal/v1/runtime/operations/{id}/cancel` | Propagate cancellation to the active lifecycle operation. |
| `POST /internal/v1/setup/plan` | Persist a concrete setup plan for one selected recommendation. |
| `POST /internal/v1/setup/jobs/recovery` | Recover the most recent relevant persisted setup job after restart. |
| `POST /internal/v1/setup/jobs/{id}/approve` | Record an explicit decision for the exact persisted plan revision. |
| `POST /internal/v1/setup/jobs` | Start one approved durable setup attempt. |
| `POST /internal/v1/setup/jobs/{id}/status` | Return the authoritative persisted setup snapshot. |
| `GET /internal/v1/setup/jobs/{id}/events?after_sequence={cursor}&limit={count}` | Stream a bounded page of persisted setup events after an exclusive cursor. |
| `POST /internal/v1/setup/jobs/{id}/cancel` | Request cooperative cancellation of an active setup attempt. |
| `POST /internal/v1/setup/jobs/{id}/retry` | Retry the exact current approved plan after preconditions are rechecked. |
| `POST /internal/v1/shutdown` | Request bounded sidecar shutdown. |

The deterministic operation is transport-foundation behavior only. It is not a product workflow and has no hardware, runtime, model, download, persistence, or chat semantics.

Hardware scans reuse the same authentication, handshake, correlation, bounded SSE, cancellation, and safe-error controls. Only one scan runs at a time, and the host retains at most eight in-process scan records for stream replay. No hardware evidence is exposed through an unauthenticated route or written to SQLite.

Capability recommendations use the same authentication, handshake, body limit, timeout, and identifier checks. The route is an in-memory bounded calculation: it consumes the supplied `MachineProfile`, does not start a scan, does not query Windows or SQLite, and exposes no raw catalogue rules. See [`capability-recommendations.md`](./capability-recommendations.md).

Runtime routes reuse the same authentication, handshake, identifier, body, timeout, and safe-error controls. The handshake supplies the opaque identity of the runtime provider registered by the Rust composition root; Flutter echoes that identity and does not select or name a provider in application code. JSON response bodies have a total configured deadline and a 64 KiB limit, including error responses. Lifecycle SSE consumption separately enforces the configured inactivity timeout, a 512 KiB cumulative body limit, per-event limits, and ordered terminal events before decoding into UI state. The routes expose normalized state and user intentions rather than provider URLs, commands, payloads, executable paths, or raw errors. The host retains at most eight lifecycle operation records and permits only one active lifecycle operation. Core policy keeps discovery, reuse consent, ownership, and management consent separate; external reuse approval permits bounded read-only model inspection but does not authorize start, stop, restart, update, uninstall, or reconfiguration. See [`ollama-runtime.md`](./ollama-runtime.md).

Setup routes are thin adapters over the Rust-owned persistent setup service. Planning records an awaiting-approval job before Flutter renders it. Approval names only the job and exact plan revision; Rust copies the stored model, provider artifact, licence, provenance, destination, expected size, and authorized effects into the durable approval record. Starting, cancelling, retrying, and recovering always return an authoritative snapshot. Provider acquisition can continue for its bounded core deadline even when the desktop detaches from the event stream.

Setup SSE uses an exclusive persisted `after_sequence` cursor and a requested `limit` from 1 through 64. Events are strictly increasing per job across retries and process restarts. Each connection is capped at 64 events and 512 KiB, uses a 16-item backpressure channel, and closes after a terminal event or its bounded page; Flutter reconnects with the last accepted cursor while the job remains active. Closing a stream, navigating away, replacing the client, or exiting the desktop only detaches the subscription. None of those actions sends cancellation. Only the explicit confirmed cancel command requests cooperative cancellation. Progress bytes and percentages are informational; `Ready`, cancellation, attention, and failure presentation always comes from persisted job state and terminal results.

## Contract generation

Rust derives JSON Schema metadata from the serialized contract types with Schemars. The repository-owned generator writes:

- `schemas/gixgiz-transport.schema.json`;
- `apps/desktop/lib/core/generated/core_contracts.g.dart`.

Generated files are marked and must not be edited by hand.

```powershell
# Regenerate both committed artifacts.
cargo run -p gixgiz-contracts --example generate_bindings -- --write

# Fail without writing when either artifact has drifted.
cargo run -p gixgiz-contracts --example generate_bindings -- --check
```

The generator intentionally supports only the contract shapes used by this internal boundary: documented string enums, objects, UUID/string aliases, integer/boolean/string fields, optionals, arrays, and references. An unsupported Rust schema shape fails generation instead of silently emitting a lossy Dart model.

## Security controls

- The listener uses IPv4 loopback and an operating-system-assigned port. No code path binds `0.0.0.0` or a LAN address.
- Authentication occurs in middleware before JSON extraction. Missing and invalid tokens receive the same safe response.
- Requests with browser `Origin` headers are rejected; no CORS headers or cookies are used.
- JSON commands require `application/json`, accept at most 16 KiB, run through a five-second request deadline, and share a 16-request concurrency bound.
- Responses and event lines are bounded by the Dart client. The foundation stream is finite, sequence-checked, correlation-checked, replayable during the process session, and must end with an explicit terminal state.
- Setup event replay is authenticated, job-ID checked, cursor-ordered, and bounded independently for every connection. Event correlation IDs identify the command or attempt that caused the event and are not replaced by the later subscription request ID.
- Runtime model inventories are authenticated, requested explicitly, capped by core policy, and never included in routine status or live-region summaries.
- Boundary failures contain stable codes, safe messages, recovery guidance, correlation IDs, and request IDs. Raw headers, tokens, provider output, panics, and stack traces are not returned.
- Structured Rust diagnostics write to stderr so stdout remains a one-record bootstrap channel. Request headers and bodies are not logged.

Loopback and a bearer token do not sandbox a fully compromised user account. A same-user process with sufficient inspection rights may observe another process. Application policy must still authorize future privileged behavior at its execution boundary.

## Validation

Run from the repository root:

```powershell
cargo run -p gixgiz-contracts --example generate_bindings -- --check
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
git diff --check
```

Run from `apps/desktop`:

```powershell
flutter pub get
flutter gen-l10n
flutter analyze
flutter test
flutter build windows
```

The Windows build requires Visual Studio 2022 Build Tools with Desktop development with C++, a Windows SDK, Flutter `3.44.8`, and the repository-pinned Rust toolchain.
