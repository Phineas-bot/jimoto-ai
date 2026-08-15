# Task 10: Implement model installation workflow

- **GitHub issue:** [#10](https://github.com/Phineas-bot/gixgiz/issues/10)
- **Depends on:** Tasks 05, 08 and 09
- **Primary decisions:** ADRs 0001–0005 plus [ADR 0006](../adr/0006-runtime-abstraction-and-ollama-provider.md) and [ADR 0007](../adr/0007-privileged-installer-helper-elevation-and-rollback.md)
- **Blocks:** Task 11

## Context and user value

A recommendation is useful only when GixGiz can safely turn it into a verified local model. This workflow must remain resumable, cancellable and honest about partial effects.

## Desired behavior

The user reviews a concrete installation plan, approves required changes, sees durable progress and reaches `Ready` only after runtime health, model availability and a real inference verification succeed.

## In scope

- Persistent setup workflow and job state machine.
- Plan review with model identity, provider artifact, licence, size, destination and estimated resource use.
- Storage preflight and selected-path availability checks.
- Explicit approval for downloads and any system-changing step.
- Resumable/cancellable Ollama model acquisition with normalized progress.
- Provider registration and model-availability verification.
- Real bounded test inference.
- Recovery after interruption or application restart.
- Retry and cleanup/retention policy for staging data.
- Model metadata persistence: canonical identity, provider ID, source, expected/measured size, lifecycle and verification state.
- Accessible Flutter progress, attention, retry and cancellation states.

## Out of scope

- Additional runtime providers.
- Full model marketplace or catalogue service.
- Automatic background update of installed models.
- Chat conversation features beyond the readiness test.
- Destructive cleanup without explicit preview and confirmation.

## Architecture constraints

- Workflow state is authoritative in Rust/SQLite.
- Model becomes `Available` only after complete verification and provider registration.
- Setup becomes `Ready` only after runtime health, model availability and real inference succeed.
- Downloads and installs are durable jobs with idempotency and cancellation semantics.
- Flutter renders persisted job state and does not infer completion from installer/provider exit alone.

## Security and privacy constraints

- Resolve trusted sources and validate available integrity metadata.
- Fail closed on identity/checksum/signature mismatch where such metadata is provided.
- Use staging paths, atomic registration and safe cleanup.
- Validate storage roots and prevent traversal/junction escape.
- Privileged steps require exact approval and the accepted helper boundary.
- Never log prompts, secrets or private content from the test inference.

## Acceptance criteria

- [X] AC-1: Plan review shows components, licence, destination, expected size and resource expectations before approval.
- [X] AC-2: Disk space and destination availability are checked before and during acquisition.
- [X] AC-3: Progress, cancellation, retry and restart recovery are persisted and displayed.
- [X] AC-4: Partial or staged artifacts are never reported as available.
- [X] AC-5: Existing compatible runtime/model assets are offered for reuse without unnecessary duplication.
- [X] AC-6: Verification includes provider registration and a real bounded inference.
- [X] AC-7: Cancellation reports completed, retained, rolled-back and uncertain effects.
- [ ] AC-8: Corrupt or mismatched artifacts are quarantined/rejected with actionable recovery. **PARTIAL** — a provider-reported integrity mismatch is rejected before readiness inference and surfaces actionable recovery, but the local catalogue carries no independently trusted expected digest, so GixGiz cannot assert independent cryptographic identity or quarantine provider-owned files in this task. See [`model-setup-workflow.md`](../guides/model-setup-workflow.md).
- [X] AC-9: Flutter covers preparing, downloading, verifying, ready, attention, failed and cancelled states accessibly.

## Required tests

- Successful clean setup using a fake provider/downloader.
- Existing compatible model reuse.
- Insufficient storage and destination disappearance.
- Interrupted/resumed download.
- User cancellation at each impactful stage.
- Provider registration failure.
- Integrity mismatch.
- Test-inference timeout/failure.
- Restart recovery and idempotent retry.
- Real Ollama smoke path on an explicitly prepared Windows environment.

## Validation commands

Run all repository validation and deterministic integration tests. Real model-download tests must be opt-in, versioned and reported separately because of network, storage and provider variability.

## Documentation updates

- Setup state machine and recovery guide.
- Model lifecycle and storage metadata.
- User-facing licence/storage explanation.
- Supported real-provider smoke procedure.

## Completion evidence

Report each workflow stage, fake and real evidence, model/provider versions, storage effects, cancellation/recovery tests, commands and any integrity limitation of the provider source.
