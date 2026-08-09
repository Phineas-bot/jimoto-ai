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
| `GET /internal/v1/hardware-scans/{id}/events` | Stream ordered typed scan events and the terminal machine profile. |
| `POST /internal/v1/hardware-scans/{id}/cancel` | Propagate cancellation to the Windows evidence process. |
| `POST /internal/v1/shutdown` | Request bounded sidecar shutdown. |

The deterministic operation is transport-foundation behavior only. It is not a product workflow and has no hardware, runtime, model, download, persistence, or chat semantics.

Hardware scans reuse the same authentication, handshake, correlation, bounded SSE, cancellation, and safe-error controls. Only one scan runs at a time, and the host retains at most eight in-process scan records for stream replay. No hardware evidence is exposed through an unauthenticated route or written to SQLite.

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
