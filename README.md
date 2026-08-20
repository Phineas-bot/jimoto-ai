# GixGiz

[![Windows CI](https://github.com/Phineas-bot/gixgiz/actions/workflows/ci.yml/badge.svg)](https://github.com/Phineas-bot/gixgiz/actions/workflows/ci.yml)

GixGiz is a desktop app that makes local AI simple. It auto-detects your PC hardware, installs the right runtime, downloads the best AI models, and gives you a GUI to use them. No technical setup is required for end users.

## One-click local setup, no CLI needed

## Current target

GixGiz v0.1 is a Windows-first foundation release that will:

1. Detect the user's hardware and relevant prerequisites.
2. Generate an explainable capability report.
3. Recommend a compatible local model configuration.
4. Detect and reuse, or explicitly install, Ollama.
5. Download and verify the selected model.
6. Provide a streaming local chat experience.

The v0.1 success condition is that a supported non-technical Windows user can move from installation to a verified local-model response through one guided graphical flow without opening a terminal.

## Status

Flutter Windows desktop shell connected to the supervised Rust platform core through a typed, authenticated, loopback-only boundary, with Rust-owned SQLite persistence, a non-elevated Windows hardware-evidence scan, deterministic local capability planning, consent-aware local runtime inspection, a durable approved model-setup workflow, and a local streaming chat workspace.

## Repository map

```text
gixgiz/
├── apps/                  # User-facing applications; Flutter desktop first
├── crates/                # Rust platform core and service modules
├── docs/
│   ├── specs/             # Product, architecture, modules, UX and release specs
│   ├── adr/               # Architecture Decision Records
│   └── tasks/             # Implementable work specifications
├── integrations/          # Future IDE and external-client integrations
├── packages/              # Future first-party AI Packs
├── sdk/                   # Future public SDK
├── tests/                 # Integration, end-to-end and security tests
├── AGENTS.md              # Authoritative coding-agent instructions
└── AGENT.md               # Convenience redirect
```

Directories are added when real implementation work requires them; the project does not create speculative empty modules.

## Documentation

- [`AGENTS.md`](./AGENTS.md) — repository-wide engineering and AI-agent policy.
- [`docs/specs/`](./docs/specs/) — authoritative product and technical specifications.
- [`docs/adr/`](./docs/adr/) — accepted architectural decisions.
- [`docs/tasks/`](./docs/tasks/) — issue-quality implementation specifications.

## Development approach

GixGiz is developed as a local-first modular monolith using vertical slices. The logical v0.1 task sequence is:

| Task | Capability |
|---:|---|
| 01 | [Establish repository and build architecture](./docs/tasks/0001-establish-repository-and-build-architecture.md) |
| 02 | [Create the Flutter Windows desktop shell](./docs/tasks/0002-create-flutter-windows-desktop-shell.md) |
| 03 | [Create the Rust workspace and platform core](./docs/tasks/0003-create-rust-workspace-and-platform-core.md) |
| 04 | [Establish typed Flutter–Rust communication](./docs/tasks/0004-establish-typed-flutter-rust-communication.md) |
| 05 | [Add the SQLite persistence foundation](./docs/tasks/0005-add-sqlite-persistence-foundation.md) |
| 06 | [Add the Windows CI pipeline](./docs/tasks/0006-add-windows-ci-pipeline.md) |
| 07 | [Implement the hardware-scan vertical slice](./docs/tasks/0007-implement-hardware-scan-vertical-slice.md) |
| 08 | [Implement the capability recommendation engine](./docs/tasks/0008-implement-capability-recommendation-engine.md) |
| 09 | [Implement the Ollama runtime adapter](./docs/tasks/0009-implement-ollama-runtime-adapter.md) |
| 10 | [Implement the model installation workflow](./docs/tasks/0010-implement-model-installation-workflow.md) |
| 11 | [Implement local streaming chat](./docs/tasks/0011-implement-local-streaming-chat.md) |

All contributors and coding agents must read [`AGENTS.md`](./AGENTS.md) before changing the repository.

## Development prerequisites

The supported implementation environment is Windows 11 x64. Development requires:

- Git.
- Flutter stable with Windows desktop support enabled.
- Visual Studio 2022 or Visual Studio Build Tools with the **Desktop development with C++** workload and a Windows SDK.
- Rustup with the pinned Rust `1.97.1` MSVC toolchain from [`rust-toolchain.toml`](./rust-toolchain.toml).

Task 02 uses Flutter `3.44.8` stable with Dart `3.12.2`; the generated project metadata records framework revision `058e0af2c2b57e369d905a03ac9748b0ebf543c6`. Cargo automatically selects the repository toolchain, including `rustfmt` and `clippy`, when Rustup is available.

## Windows CI

Pull requests, pushes to `main`, and manual runs execute the same Rust, binding,
persistence, Flutter, and Windows build checks documented below. The workflow
uses pinned toolchains, read-only repository permissions, and no secrets. A
successful run retains an unsigned diagnostic Windows bundle for seven days;
it is not a signed or published release.

Run the complete local equivalents before opening a pull request:

```powershell
cargo run -p gixgiz-contracts --example generate_bindings -- --check
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features

Set-Location apps/desktop
flutter pub get
flutter gen-l10n
flutter analyze
flutter test
flutter build windows
```

Cache boundaries, action pins, deterministic test policy, and artifact details
are documented in [`docs/guides/windows-ci.md`](./docs/guides/windows-ci.md).

## Rust platform foundation

The root Cargo workspace contains six crates with a strict dependency direction:

```text
gixgiz-desktop-host -> gixgiz-runtime-ollama -> gixgiz-runtime -> gixgiz-contracts
                    -> gixgiz-core -----------^         |
                                      -> gixgiz-persistence -> gixgiz-contracts
```

- `gixgiz-contracts` owns serializable, provider-neutral identity, readiness, health, machine-profile, capability-plan, runtime, setup-job, handshake, event, cancellation, error, recovery, and request-correlation contracts.
- `gixgiz-persistence` exclusively owns the SQLite connection, data-root layout, migrations, backups, health checks, setup/model records, and typed repository SQL.
- `gixgiz-runtime` owns provider-neutral runtime and model-setup traits, normalized adapter failures, cancellation/deadline conventions, and deterministic fake providers.
- `gixgiz-runtime-ollama` owns Ollama executable discovery, loopback API details, version and model payloads, owned-child lifecycle control, allowlisted model acquisition, fixed readiness verification, and provider error normalization.
- `gixgiz-core` owns platform lifecycle, deterministic readiness policy, persistence composition, runtime ownership/consent authorization, durable setup orchestration, hardware-scan orchestration, Windows evidence normalization, capability filtering/scoring, safe error mapping, diagnostics initialization, cancellation, and timeout conventions.
- `gixgiz-desktop-host` builds `gixgiz-core.exe`, composes the concrete v0.1 adapter, reads a per-launch secret from the inherited stdin pipe, initializes core services on blocking workers, and exposes only authenticated provider-neutral HTTP/SSE routes on a dynamic `127.0.0.1` port. It exposes safe persistence health, runtime status, typed hardware evidence, and capability reports but no provider proxy, paths, SQL, or database access.

Run the Rust checks from the repository root:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

The Rust contracts generate both [`schemas/gixgiz-transport.schema.json`](./schemas/gixgiz-transport.schema.json) and the checked Dart models. Regenerate or verify them from the repository root:

```powershell
cargo run -p gixgiz-contracts --example generate_bindings -- --write
cargo run -p gixgiz-contracts --example generate_bindings -- --check
```

Transport architecture, bootstrap, routes, security controls, and development checks are documented in [`docs/guides/flutter-rust-transport.md`](./docs/guides/flutter-rust-transport.md).

## Local runtime adapter

The v0.1 composition root registers one Ollama adapter behind provider-neutral core contracts. Detection verifies an approved loopback endpoint, executable evidence when present, provider health, and semantic version evidence without adopting an external installation. Read-only reuse requires an explicit persisted reuse decision; management ownership and consent remain separate, and external start, stop, restart, update, uninstall, or reconfiguration are not authorized by reuse approval.

The adapter uses bounded direct HTTP and structured process execution without a shell. It reports normalized status and inventory, and Task 10 permits only allowlisted model acquisition plus a fixed bounded readiness inference behind an exact setup approval. It does not install, update, reconfigure, or uninstall Ollama; delete models; expose arbitrary generation; or run chat. Architecture, version policy, endpoint rules, ownership behavior, troubleshooting, and opt-in smoke tests are documented in [`docs/guides/ollama-runtime.md`](./docs/guides/ollama-runtime.md).

## Windows hardware evidence

The Foundation screen can request a versioned `MachineProfile` from Rust. The scanner uses fixed, non-elevated inbox Windows PowerShell and CIM queries for a narrow allowlist: operating-system version and architecture, CPU identity and core counts, total and available physical memory, display-adapter names/vendors, and capacity metadata for the drive containing `%LOCALAPPDATA%`. Flutter only renders the typed result.

Unreported, inaccessible, or unreliable values remain explicitly unknown with source, availability, confidence, and a safe reason. In particular, Windows display-adapter memory and acceleration support are not inferred from marketing names. The scan collects no serial numbers, MAC addresses, PNP IDs, file listings, or unrelated stable identifiers; it is not persisted or uploaded. See [`docs/guides/windows-hardware-scan.md`](./docs/guides/windows-hardware-scan.md).

## Capability recommendations

After a scan, Flutter can ask Rust for a deterministic plan using beginner-facing workload and resource preferences. Rust applies hard safety constraints before stable preference scoring and returns a recommended plan, smaller fallback, optional larger choice, conservative resource allowances, confidence, reasons, warnings, and an explicit no-plan result when critical evidence or headroom is insufficient. Unknown video memory and acceleration are never guessed.

The local static catalogue and rule set are versioned in every report and plan. The operation performs no download, installation, runtime call, benchmark, LLM decision, database write, or machine-data upload. Catalogue metadata, estimate assumptions, terminology, persistence deferral, and limitations are documented in [`docs/guides/capability-recommendations.md`](./docs/guides/capability-recommendations.md).

## Persistent model setup

An accepted recommendation can be converted into a concrete, persisted setup plan that shows the canonical model, provider artifact, licence, provenance, provider-managed destination, expected size, resource estimates, warnings, and exact material effects before approval. Approval is bound to one immutable plan revision and does not transfer ownership of an external runtime or authorize runtime installation, update, deletion, or general inference.

Rust and SQLite own every setup transition. The provider acquisition stream is normalized and bounded, cancellation records completed, retained, rolled-back, and uncertain effects, and interrupted jobs recover as explicit attention instead of silently resuming a provider mutation. A model becomes `Available` and setup becomes `Ready` only after runtime health, exact model availability, provider registration, and a fixed bounded readiness inference all succeed. See [`docs/guides/model-setup-workflow.md`](./docs/guides/model-setup-workflow.md).

## Local streaming chat

After a verified setup, the chat workspace creates local conversations, streams assistant replies incrementally, and stops generation on request. Rust and SQLite own every conversation and message; Flutter renders persisted state and never calls a provider, opens the database, or infers completion.

Replies reach `Ready` only from persisted core state. Stopping a reply produces a distinct cancelled state whose retained text is presented as incomplete, and a reply interrupted by an earlier exit can never recover as completed. Conversation deletion removes GixGiz-owned records only; it never removes the runtime, the model, provider-owned files, or other conversations. Messages stay on the device, and default logs exclude prompts, replies, and context. See [`docs/guides/local-chat.md`](./docs/guides/local-chat.md).

## SQLite persistence

The Windows data root is `%LOCALAPPDATA%\GixGiz`; the database is stored at `data\gixgiz.db` beneath that root. The Rust core is the sole database owner. Flutter, transport clients, providers, Packs, and integrations never open or query SQLite directly.

Database startup enables foreign keys, WAL, and a five-second busy timeout before applying ordered transactional migrations. `PRAGMA user_version` and the immutable `schema_migrations` ledger track compatibility. Newer schemas fail closed without downgrade. Irreversible migrations require a verified online backup under `backups\` before execution.

The foundation stores bounded text metadata for application state, non-secret settings, durable-job state, categorical audit events, provider-neutral runtime ownership/consent policy, setup approvals/events/effects, canonical/provider model metadata, and local conversations and messages. It does not store detection evidence, executable or model-storage paths, provider payloads, secrets, transport tokens, prompts, generated output, conversations, logs, model binaries, partial downloads, installer files, or large blobs. Development and migration policy are documented in [`docs/guides/sqlite-persistence.md`](./docs/guides/sqlite-persistence.md).

## Initial development workflow

Clone the repository and inspect the focused task before making changes:

```powershell
git clone https://github.com/Phineas-bot/gixgiz.git
Set-Location gixgiz
git status --short
```

For repository-level hygiene, run:

```powershell
git diff --check
git -c core.quotepath=false ls-files | Where-Object { $_.Contains([char]0x200B) }
```

The second command must produce no output. Also verify the Markdown navigation links manually. Each task specification defines its exact targeted commands; once a repository task runner is introduced, its documented commands become the validation source of truth.

For the Task 02 desktop shell, use the exact setup, analysis, test, run, and build commands in [`apps/desktop/README.md`](./apps/desktop/README.md).

## Technology direction

- **Desktop UI:** Flutter
- **Core platform:** Rust
- **Local persistence:** SQLite
- **Initial runtime provider:** Ollama
- **Initial platform:** Windows 11 x64
- **Automation:** GitHub Actions

These choices are governed by the accepted foundation Architecture Decision Records.

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for local setup, validation,
security, and pull-request expectations. The repository is in its foundation
stage: link work to a focused issue with explicit scope, non-goals, acceptance
criteria, tests, and validation evidence. Do not submit broad "build the
platform" changes.

## Licence

GixGiz is available under the [MIT License](./LICENSE).
