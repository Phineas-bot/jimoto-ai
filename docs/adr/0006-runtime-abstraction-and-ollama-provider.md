# ADR 0006: Runtime abstraction and Ollama provider ownership

- **Status:** Accepted
- **Date:** 2026-08-09
- **Owners:** GixGiz project owner
- **Decision scope:** Runtime abstraction, provider registration, lifecycle state, and ownership; required before Tasks 09-10
- **Related:** ADR 0001, ADR 0002, ADR 0003, ADR 0005, GitHub issues #9 and #10

## Context

GixGiz must turn a provider-neutral capability recommendation into a usable local runtime without making the product an Ollama-specific frontend. Ollama is the first v0.1 provider, but its executable names, API routes, payloads, installation layout, process behavior, errors, and model tags are implementation details that can change independently from GixGiz contracts and user workflows.

The platform also needs to distinguish discovering a compatible installation from receiving consent to manage it. A healthy runtime installed by the user or another application may be reusable, but discovery alone cannot authorize GixGiz to start, stop, update, reconfigure, adopt, or uninstall it.

Task 09 needs an explicit boundary before adding process or provider behavior. Task 10 then needs stable runtime and model identities for durable setup work without persisting Ollama-specific policy throughout the platform.

## Decision

### 1. Provider-neutral abstraction

GixGiz will define a provider-neutral runtime abstraction above every runtime adapter.

- Shared serialized contracts describe runtime identity, capabilities, ownership, normalized state, health, versions, operations, model identities, and safe failures without provider product names.
- Application policy consumes the abstraction and does not depend on Ollama commands, endpoints, payloads, process rules, errors, or model tags.
- The concrete Ollama adapter translates between normalized operations and Ollama behavior.
- Unsupported or unknown capabilities remain explicit. The abstraction does not pretend every provider supports every lifecycle or model operation.

Task 09 will introduce the smallest focused logical boundaries needed for this design:

- a provider-neutral runtime API boundary for in-process traits and normalized results;
- an Ollama adapter boundary that implements that API;
- core orchestration that depends on the runtime API rather than the Ollama implementation; and
- compile-time composition that registers the Ollama adapter for v0.1.

These logical boundaries should be represented by focused Rust crates when Task 09 implements them, following repository naming and dependency policy. No dynamic plugin loader or public provider SDK is introduced for v0.1.

### 2. Ollama is the first registered provider

Ollama is the only runtime provider registered by the v0.1 composition root. Registration is explicit and compile-time; GixGiz does not discover arbitrary provider plugins or load untrusted dynamic libraries.

All of the following stay inside the Ollama adapter boundary:

- executable, service, process, and installation discovery rules;
- command names and structured argument lists;
- local API addresses, routes, request and response payloads;
- provider version parsing and compatibility rules;
- provider process startup, supervision, and shutdown behavior;
- provider error parsing and redaction;
- Ollama model names, tags, digests, and list-response details; and
- provider-specific capability and limitation detection.

Flutter, Packs, external clients, persistence repositories, and capability recommendation rules never call Ollama directly. The authenticated desktop transport may expose provider-neutral runtime intentions and state only after those contracts are added by Task 09.

### 3. Normalized runtime state

The runtime abstraction represents at least these states:

- `NotInstalled`: no compatible installation was verified;
- `InstalledStopped`: a compatible installation exists but is not ready;
- `Starting`: a bounded start operation is in progress;
- `Ready`: version, endpoint, and health verification succeeded;
- `Degraded`: the runtime responds with reduced or uncertain capability;
- `Incompatible`: an installation exists but its verified version or capability is unsupported;
- `Updating`: an update is observed or managed by an explicitly approved future workflow; and
- `Failed`: a lifecycle operation or runtime process reached a terminal failure.

State is based on current evidence, not optimistic UI state or one process exit code. Unknown evidence and unsupported capabilities remain explicit in the health detail or safe failure instead of being coerced into `Ready`.

Task 09 may observe `Updating`, but it does not implement provider update behavior.

### 4. Runtime ownership and consent

Runtime installation ownership is recorded independently from detection and health:

- `External`: installed or managed outside GixGiz;
- `GixGizManaged`: installed or registered by a GixGiz workflow after explicit user approval;
- `Bundled`: shipped and managed as part of GixGiz if a later packaging decision permits it; and
- `Unknown`: ownership cannot be established safely.

Detection never changes ownership. Reuse consent and management ownership are separate records and separate user decisions.

- A compatible external installation may be offered for reuse.
- Read-only detection, version, health, and model-list inspection do not adopt the installation.
- Starting, stopping, or restarting an external runtime requires informed user consent appropriate to the operation and must not silently change its installation, update channel, service registration, configuration, or data.
- GixGiz never silently updates, uninstalls, replaces, reconfigures, or claims ownership of an external installation.
- A future managed installation must record the exact approved scope and preserve enough metadata to distinguish GixGiz-owned effects from pre-existing user assets.

### 5. Bounded lifecycle operations

Runtime operations use typed requests and `OperationContext`-style correlation, cancellation, and deadlines.

- Executables and endpoints are validated before use.
- Process execution passes an executable and arguments separately and never interpolates untrusted text into a shell command.
- Provider connections are limited to approved local interfaces. A discovered non-loopback endpoint is not trusted as the local managed runtime.
- Start, stop, restart, version, health, and model-list operations have fixed timeouts and bounded output.
- Restart policy is bounded and cannot create an infinite recovery loop.
- Cancellation is distinct from failure and reports completed, retained, and uncertain effects.
- Logs contain stable operation, state, version, and correlation metadata, but not bearer tokens, prompts, conversation content, private files, arbitrary environment variables, or raw provider output.
- Provider failures map to stable provider-neutral error categories and safe recovery guidance; raw Ollama errors remain diagnostic causes inside the adapter.

### 6. Provider-specific model identity mapping

GixGiz keeps canonical catalogue/model identity separate from provider identifiers.

- Recommendation and setup policy use provider-neutral catalogue and model IDs.
- The Ollama adapter maps an approved canonical model identity to or from an Ollama name, tag, or digest.
- Provider identifiers are retained only where required for adapter operations, verification, and bounded diagnostics.
- A provider tag alone does not prove canonical identity, integrity, availability, or readiness.
- Task 09 may list and normalize installed model metadata. It does not pull, download, delete, or verify inference readiness for a selected model.

### 7. Task boundaries

Task 09 may implement:

- installation and endpoint detection;
- provider version and compatibility inspection;
- normalized health and state;
- explicitly consented start, stop, and bounded restart;
- model listing and provider-to-canonical identifier mapping; and
- deterministic fake-adapter tests plus separately marked opt-in real-Ollama smoke tests.

Task 09 must not implement:

- model pull or download;
- chat, generation, or inference streaming;
- silent runtime adoption, update, reconfiguration, or uninstall;
- automatic privileged installation;
- a public provider API, dynamic plugin system, or additional runtime provider; or
- privileged behavior not explicitly permitted by ADR 0007 and the active task scope.

## Alternatives considered

### Call Ollama directly from Flutter

Rejected. It would move provider and process policy into presentation code, bypass core authorization and diagnostics, and make future clients duplicate unsafe behavior.

### Expose Ollama routes through the sidecar unchanged

Rejected. A transparent proxy would leak provider contracts, enlarge the authenticated transport surface, and prevent GixGiz from enforcing ownership, policy, cancellation, and safe error normalization.

### Put Ollama-specific types in shared contracts

Rejected. It would make recommendations, persistence, UI, and future adapters depend on one provider's terminology and versioning.

### Adopt any detected compatible installation automatically

Rejected. Compatibility evidence does not grant management authority. Silent adoption could disrupt software and model data controlled by the user or another application.

### Build a dynamic runtime plugin ecosystem now

Deferred. v0.1 has one provider and no public SDK. Dynamic loading would add signing, ABI, sandbox, registration, and supply-chain risks without current product value.

### Bundle Ollama immediately

Deferred. Bundling changes packaging, licensing, updates, storage, and rollback. ADR 0005 intentionally leaves the final installer format open, and ADR 0007 requires a narrow privilege boundary for system-changing work.

## Consequences

### Positive

- Core workflows remain provider-neutral while Ollama can evolve independently.
- Ownership and consent are explicit instead of inferred from discovery.
- Lifecycle behavior has one normalized state and error model.
- Fake adapters can provide deterministic CI coverage without a live provider.
- A future provider can implement the same abstraction without changing Flutter or setup policy wholesale.

### Negative

- Task 09 needs mapping code between normalized contracts and Ollama behavior.
- Provider limitations cannot be hidden behind a falsely universal interface.
- Compile-time registration requires a new build to add another provider.
- External installations require consent and ownership-state handling even when technically compatible.

## Security and privacy impact

- Flutter and untrusted local clients cannot invoke Ollama directly through GixGiz.
- Detection validates executable identity, canonical paths, local endpoints, and provider versions before execution or connection.
- Non-elevated inspection is the default; privileged installation is outside Task 09.
- Process arguments, output, and logs are bounded and sanitized.
- Provider model listings may reveal local model names, so they cross only authenticated boundaries and are not logged or uploaded by default.
- External installations and their data remain externally owned unless the user explicitly approves a managed transition in a later workflow.

## Implementation constraints

- Task 09 must preserve a provider-neutral shared contract and core API.
- The Ollama adapter must not depend on Flutter, the desktop transport, or persistence schema details.
- Core orchestration must be testable with fake runtime adapters and injected time/process/network boundaries.
- The desktop host may compose the concrete v0.1 adapter, but it must not acquire runtime policy.
- Any added serialized enums require forward-compatible unknown handling and regenerated checked bindings.
- Any persisted ownership or consent record requires an ordered migration and separate detection, ownership, and approval fields.
- Real Ollama tests are opt-in, version-recorded, and separate from required deterministic CI.
- No runtime command, endpoint, supported version range, or model tag is decided by this ADR; Task 09 must document and test the exact supported implementation values.

## Follow-up work

- Task 09 implements the provider-neutral runtime API and Ollama adapter within these boundaries.
- Task 09 documents supported Ollama versions, local endpoint policy, lifecycle limitations, and opt-in smoke-test setup.
- Task 10 persists approved setup/job state, acquires and verifies models, and performs a bounded real inference readiness test.
- ADR 0007 governs any privileged helper used by later runtime installation or system-changing workflows.
- A later ADR is required before dynamic providers, a public runtime SDK, remote runtimes, or bundled-provider update policy.
