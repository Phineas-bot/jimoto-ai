# ADR 0007: Privileged installer helper, elevation, and rollback

- **Status:** Accepted
- **Date:** 2026-08-09
- **Owners:** GixGiz project owner
- **Decision scope:** Privileged installation and system-change boundary; required before Tasks 09-10
- **Related:** ADR 0001, ADR 0004, ADR 0005, ADR 0006, GitHub issues #9 and #10

## Context

GixGiz is a per-user Windows application whose desktop and Rust core normally run without administrator rights. Detection, health checks, model listing, local inference, and reuse of a compatible external runtime do not inherently require elevation.

Some future runtime installation, repair, service registration, or system-wide component changes may require administrator privileges. Elevating the full desktop or core would give UI, transport, provider, persistence, and long-running workflow code unnecessary authority. An arbitrary elevated command runner would be equally unsafe and would make user approval, auditing, cancellation, and rollback impossible to reason about.

Task 09 must not improvise privileged installation. Task 10 and later setup workflows need a defined boundary so an approved system change can be implemented without expanding normal GixGiz authority or modifying unrelated user software.

## Decision

### 1. Normal operation remains non-elevated

`GixGiz.exe` and `gixgiz-core.exe` run as the signed-in user by default.

The following operations do not request administrator rights:

- detecting an existing runtime through approved locations and local interfaces;
- reading a provider version and compatible health state;
- listing provider models through an approved local endpoint;
- using an existing compatible runtime after informed consent;
- downloading into an approved per-user staging/content root; and
- normal per-user persistence, logs, configuration, and diagnostics.

An operation that can succeed safely per-user must not be elevated for convenience.

### 2. Narrow short-lived helper boundary

Any operation that genuinely requires administrator rights runs through a dedicated, short-lived privileged helper.

- The helper is launched only for one explicitly approved operation and exits after returning a result.
- It is not the Flutter desktop, Rust core, runtime adapter, updater, or a general-purpose process tool.
- It is not installed as a permanent background service in v0.1.
- It exposes no network listener and accepts no browser, Pack, external-client, or provider-originated request.
- It does not accept arbitrary executable paths, shell strings, scripts, environment blocks, working directories, registry fragments, or unbounded argument lists.
- It implements a small allowlist of versioned structured operation kinds compiled into the helper.

The normal core owns workflow policy and decides whether an approved operation is needed. The helper independently validates the structured request and fails closed if the operation, version, identity, path, precondition, or requested effect is outside its allowlist.

### 3. Structured request and result

Each helper request is bounded, versioned, and contains at least:

- one allowlisted operation kind;
- the exact component identity and expected version or version range;
- canonical expected source, staging, destination, and affected paths where applicable;
- expected integrity or package identity evidence;
- explicit preconditions and postconditions;
- an operation-specific rollback or recovery plan;
- the user-approved effect summary and approval record reference;
- correlation, request, durable-job, and sanitized audit identifiers;
- a deadline and cancellation semantics; and
- an upper bound on retained output and diagnostics.

The request travels through a bounded local mechanism that does not expose secrets in process arguments or URLs. The helper returns a structured result containing status, verified effects, retained effects, rolled-back effects, uncertain effects, safe error classification, and bounded sanitized diagnostics.

The exact Windows invocation and private IPC mechanism are selected during helper implementation and must receive security review. They must preserve the constraints in this ADR and the package identity in ADR 0005.

### 4. Explicit approval and elevation

Windows elevation consent is necessary but not sufficient approval.

Before launching the helper, GixGiz must show and record:

- the exact requested operation;
- why elevation is required;
- the component and version involved;
- the paths or system components expected to change;
- storage and other material effects;
- the rollback or recovery plan; and
- any known effect that cannot be rolled back automatically.

Approval is scoped to one operation. It cannot authorize arbitrary later work, be silently reused for another component, or become a persistent elevation token. A changed plan, path, package, version, or expected effect requires a new approval.

### 5. Staging, integrity, and execution

Downloads and package preparation occur without elevation in a validated staging root whenever possible. The helper performs only the minimal privileged commit step.

- The helper verifies expected package identity and available checksum or signature evidence before changing the system.
- It canonicalizes and authorizes every path against the operation's allowlist.
- It uses structured operating-system or installer APIs where available.
- If a child process is unavoidable, the helper resolves an allowlisted executable and passes a fixed validated argument schema without a shell.
- Execution has a deadline, bounded output, and resource limits appropriate to the operation.
- Logs include correlation and categorical progress but exclude credentials, tokens, private content, raw environment data, and unbounded installer output.

### 6. Cancellation, partial effects, and rollback

Privileged operations are cancellable at safe checkpoints. Cancellation is not treated as failure.

- Before the commit point, cancellation should leave no system change beyond removable operation-owned staging.
- During a non-interruptible system transaction, GixGiz may defer cancellation until a safe checkpoint rather than corrupt the installation.
- After any interruption, the helper reports what completed, what was rolled back, what was retained for recovery, and what remains uncertain.
- Rollback uses the operation manifest and may remove or restore only effects created or replaced by that approved operation.
- Rollback never deletes unrelated user files, external model data, external runtime data, or an existing external Ollama installation.
- A destructive or irreversible step requires a verified recovery path before execution; otherwise the helper refuses the operation.
- Failed rollback preserves evidence and returns an attention-required state rather than claiming success.

### 7. External runtime protection

An external Ollama installation remains externally owned under ADR 0006.

- Detection and health do not invoke the privileged helper.
- The helper cannot update, repair, uninstall, relocate, reconfigure, or register over an external installation unless a later task defines an explicit ownership transition and the user approves that exact transition.
- A GixGiz-managed installation records the files, registrations, versions, and other effects GixGiz owns so repair and rollback stay within that manifest.
- Uninstalling GixGiz does not silently invoke the helper to remove external software or user data.

### 8. Task boundaries

Task 09 does not implement automatic privileged installation or invoke a helper for detection, health, lifecycle inspection, or model listing. If an installation is absent and setup requires elevation, Task 09 reports a provider-neutral attention state and defers the system change.

Task 10 may use this ADR only when its approved implementation scope includes the required helper capability. Model acquisition should remain non-elevated when it targets an approved per-user or user-selected storage root. Any runtime installation or system-changing setup step still requires the helper, exact approval, durable job state, and rollback evidence.

This ADR does not select the final Windows installer format, automatic update mechanism, or release signing workflow.

## Alternatives considered

### Elevate the desktop or core process

Rejected. It would grant presentation, transport, database, provider, and long-running orchestration code unnecessary administrator authority and increase the impact of any defect or compromised local client.

### General-purpose elevated command or PowerShell runner

Rejected. Arbitrary commands cannot be safely authorized by a narrow user approval, are difficult to validate and roll back, and create a direct command-injection boundary.

### Permanent privileged Windows service

Rejected for v0.1. A service adds installation, authentication, lifetime, update, multi-user, and attack-surface complexity. Current operations do not justify persistent privilege.

### Let each runtime adapter request elevation directly

Rejected. It would duplicate privilege and rollback policy in provider code and make the adapter an authority boundary instead of a translation boundary.

### Require all setup to be per-user and never support elevation

Not selected as a universal rule. Per-user operation is preferred, but some supported third-party installation modes may require system changes. Those operations need a safe narrow path rather than an implicit unsupported workaround.

### Choose MSIX, MSI, or a custom bootstrapper in this ADR

Deferred. ADR 0005 requires evidence from real packaging and upgrade behavior before selecting the final installer and update format.

## Consequences

### Positive

- Normal application code retains least privilege.
- User approval is specific, reviewable, and tied to actual expected effects.
- The helper surface can be allowlisted, tested, audited, and code-reviewed independently.
- Cancellation and rollback semantics are explicit before system-changing work begins.
- External installations and unrelated user files are protected from broad cleanup.

### Negative

- Privileged workflows require an additional executable, private request channel, packaging, signing, and security tests.
- Some cancellation requests must wait for a safe checkpoint.
- Every helper operation needs its own validation, effect manifest, recovery design, and tests.
- Development builds cannot provide release-grade package identity evidence until signing and installer work is available.

## Security and privacy impact

- Administrator authority is absent from normal desktop, core, transport, persistence, recommendation, and runtime-inspection paths.
- The helper accepts only local, bounded, structured, allowlisted operations and exposes no general shell or network service.
- User approval, Windows elevation, request validation, and operation allowlisting are separate checks.
- Canonical path validation and an owned-effect manifest prevent traversal and unrelated deletion during rollback.
- Staged packages are integrity-checked before privileged use.
- Audit and diagnostic data are bounded and sanitized; secrets, prompts, conversations, personal files, tokens, and raw installer output are excluded.
- A compromised provider process cannot directly instruct the helper.

## Implementation constraints

- No helper operation may accept arbitrary command text or user-supplied executable paths.
- The helper must forbid `unsafe` Rust unless a later accepted ADR documents an unavoidable isolated use.
- Requests and results require explicit schema versions and forward-incompatible operations fail closed.
- The helper executable and request channel require package-relative identity, same-user caller, replay, tamper, and spoofing analysis before implementation.
- Tests must cover invalid operation kinds, stale or replayed approval, path escape, identity mismatch, integrity failure, permission denial, timeout, safe cancellation, partial failure, rollback, failed rollback, and output redaction.
- Real elevation tests run only in an explicitly prepared Windows environment and remain separate from deterministic CI.
- The helper must never delete files not recorded in the approved operation manifest.
- Task 09 remains non-privileged and may not add an installation shortcut around this boundary.

## Follow-up work

- Define the exact helper executable identity, Windows launch mechanism, private IPC, replay protection, and signing verification during the first approved helper implementation.
- Add a threat model and independent security review before shipping privileged operations.
- Task 10 defines durable setup stages, approvals, model acquisition, verification, cancellation, and recovery around the runtime abstraction.
- Select the final Windows installer and automatic update strategy in a separate ADR after packaging evidence exists.
- Define repair and uninstall UX that distinguishes GixGiz-owned effects, GixGiz user data, and external runtime/model assets.
