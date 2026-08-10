# Task 09: Implement Ollama runtime adapter

- **GitHub issue:** [#9](https://github.com/Phineas-bot/gixgiz/issues/9)
- **Depends on:** Tasks 03–08
- **Requires before implementation:** [ADR 0006](../adr/0006-runtime-abstraction-and-ollama-provider.md) and [ADR 0007](../adr/0007-privileged-installer-helper-elevation-and-rollback.md)
- **Blocks:** Tasks 10 and 11

## Context and user value

GixGiz needs one managed local runtime to turn recommendations into working AI. Ollama is the first provider, but its commands, endpoints and lifecycle semantics must remain isolated behind normalized platform contracts.

## Desired behavior

GixGiz can detect an existing Ollama installation, report normalized health/version state, reuse compatible external installations with informed consent, and perform bounded lifecycle/model-list operations through a dedicated adapter.

## In scope

- Runtime abstraction and capability descriptor required by v0.1.
- `runtime-ollama` implementation isolated from shared contracts.
- Detection of executable/service/API and provider version.
- Normalized states: `NotInstalled`, `InstalledStopped`, `Starting`, `Ready`, `Degraded`, `Incompatible`, `Updating`, `Failed`.
- Existing-installation ownership classification: external, user-approved managed or bundled/managed where applicable.
- Start, stop, restart, health and version operations.
- Model listing and provider identifier mapping.
- Provider error normalization, timeouts, cancellation and bounded logs.
- Fake-provider contract tests and separately marked real-Ollama smoke tests.
- Flutter runtime status and recovery actions.

## Out of scope

- Automatic privileged installation before the helper ADR/implementation is approved.
- Model download workflow.
- Chat UI or conversation persistence.
- Additional runtime adapters.
- Silent update, uninstall or adoption of externally managed Ollama.

## Architecture constraints

- All Ollama-specific endpoints, commands, payloads and error mapping stay inside `runtime-ollama`.
- Shared domain types are provider-neutral.
- Flutter and Packs never call Ollama directly.
- Unsupported provider capabilities are explicit.
- Runtime ownership and management consent are persisted separately from detection.

## Security and privacy constraints

- Validate executable identity/path and provider version before execution.
- Use structured process arguments; no shell interpolation.
- Bind/connect only to approved local interfaces.
- Every operation has timeout, cancellation and output bounds.
- Do not expose raw provider errors as the only user message.
- Never silently update, uninstall or change an external runtime.
- Privileged operations require a narrow helper and exact per-operation approval.

## Acceptance criteria

- [ ] AC-1: Detection distinguishes absent, stopped, ready, degraded and incompatible installations.
- [ ] AC-2: All required normalized runtime states are represented and tested.
- [ ] AC-3: Existing compatible installations are offered for reuse without silent ownership changes.
- [ ] AC-4: Provider-specific details do not leak above the adapter boundary.
- [ ] AC-5: Lifecycle and health calls are bounded, cancellable and error-normalized.
- [ ] AC-6: Local endpoint safety and version compatibility are verified.
- [ ] AC-7: Fake-provider contract tests are deterministic; real Ollama smoke tests are optional/separate.
- [ ] AC-8: Flutter presents actionable runtime states and management ownership clearly.

## Required tests

- No installation present.
- Executable present/service stopped.
- Healthy compatible provider.
- Unsupported/incompatible version.
- Reachable but degraded provider.
- Startup timeout, crash, cancellation and bounded restart.
- External versus managed ownership behavior.
- Error mapping and redaction.
- Model-list normalization.

## Validation commands

Run the complete Rust, Flutter, contract, migration and Windows CI-equivalent command set. Run real Ollama smoke tests only on an explicitly prepared environment and report the provider version.

## Documentation updates

- Runtime abstraction and ownership model.
- Supported Ollama versions/capabilities.
- Troubleshooting and external-installation behavior.
- Required new ADRs.

## Completion evidence

Report provider versions tested, ownership scenarios, normalized states, real versus fake evidence, commands, security checks and any installation operation intentionally deferred.
