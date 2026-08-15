# AGENTS.md - Rust workspace policy

> Applies to all crates under `crates/` in addition to the repository root policy.

## Ownership and dependency direction

- `gixgiz-contracts` owns versioned, provider-neutral data shared across process boundaries. It must not depend on the core or host.
- `gixgiz-persistence` exclusively owns SQLite connections, migrations, backups, setup/model records, and repository SQL. It may depend on contracts but must not depend on core, host, or Flutter.
- `gixgiz-runtime` owns provider-neutral runtime and model-setup traits, operation control, normalized adapter errors, and deterministic test support. It may depend on contracts but not on a concrete provider, persistence, the host, or Flutter.
- `gixgiz-runtime-ollama` owns all Ollama-specific paths, commands, endpoints, payloads, version parsing, model identifiers, model acquisition, fixed readiness verification, process behavior, and provider-error normalization. It may depend on runtime and contracts only.
- `gixgiz-core` owns platform lifecycle, readiness policy, typed failures, operation conventions, persistence composition, runtime ownership/consent policy, durable setup authorization/state, and provider-neutral runtime orchestration. It may depend on persistence, runtime, and contracts, but not on a concrete provider, desktop host, or Flutter.
- `gixgiz-desktop-host` owns the supervised sidecar process bootstrap, compile-time concrete adapter composition, and authenticated loopback transport. It may depend on core, runtime adapters, and contracts, but must not contain platform policy.
- Keep provider implementations below the runtime abstraction: `gixgiz-desktop-host -> gixgiz-core -> gixgiz-runtime -> gixgiz-contracts`, with the host composing `gixgiz-runtime-ollama`; persistence remains an independent Rust-owned dependency of core.

## Transport boundaries

- Bind internal HTTP only to `127.0.0.1` on a dynamic port and authenticate before processing privileged bodies.
- Keep per-launch bootstrap tokens out of arguments, URLs, logs, persistence, and error text.
- Rust contracts and the generated JSON Schema are authoritative. Regenerate checked Dart bindings instead of editing them.
- Keep raw provider routes, commands, executable or storage paths, payloads, database paths or SQL, chat, and installer behavior out of the transport. Provider-neutral runtime status, consent, bounded lifecycle events, normalized model inventory, and durable setup intentions/snapshots may cross only through generated authenticated contracts.
- Do not add `unsafe` Rust.
- Tests may bind ephemeral loopback listeners, but must not use external network services or user data.
- Logs and boundary errors may include stable identifiers, state, and version metadata, but not secrets, prompts, private content, or raw provider errors.

## Validation

Run from the repository root:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo run -p gixgiz-contracts --example generate_bindings -- --check
```
