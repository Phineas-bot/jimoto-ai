# Model setup workflow

Task 10 turns one deterministic capability recommendation into a durable,
reviewable model setup. Rust and SQLite own the workflow. Flutter renders
authoritative snapshots and sends typed intentions; it does not download a
model, call a runtime, open SQLite, or infer readiness from progress.

This workflow prepares a model for local use. It does not install or update a
runtime, adopt an external runtime, delete provider data, expose general
generation, or create chat history.

## Ownership and trust boundaries

- `gixgiz-contracts` defines provider-neutral setup plans, job snapshots,
  progress, approval, effects, recovery, model lifecycle, and transport types.
- `gixgiz-persistence` owns the setup schema, transactional transitions,
  durable events, approvals, model metadata, and recovery queries.
- `gixgiz-runtime` defines capability-oriented model preparation,
  acquisition, inspection, storage preflight, and readiness-verification
  operations.
- `gixgiz-runtime-ollama` alone owns provider tags, API routes, payloads,
  streaming progress parsing, provider storage evidence, and the fixed
  readiness inference.
- `gixgiz-core` validates the recommendation, creates the review plan,
  authorizes effects, runs the state machine, persists progress, and enforces
  every readiness gate.
- `gixgiz-desktop-host` exposes only authenticated, bounded, provider-neutral
  setup operations.
- Flutter displays the persisted plan and job state through `CoreClient`.

Raw provider payloads, raw errors, private paths, prompts, generated text,
logs, secrets, model binaries, and partial provider artifacts do not cross the
adapter boundary or enter SQLite.

## Plan and exact approval

The core revalidates the selected recommendation against its local catalogue
and rule set before it creates a plan. The review includes:

- canonical model identity, display name, family, and size class;
- the opaque provider artifact mapping and runtime identity;
- licence and provenance;
- conservative expected size and resource estimates;
- a safe destination category and display label;
- warnings, limitations, and exact material effects;
- catalogue, rule-set, plan, and setup-schema versions.

The plan is persisted in `AwaitingApproval`. An approval request identifies
only the job, the exact plan revision, and the decision. Rust copies the
immutable approved scope from SQLite into the approval record. That record
contains the selected model and provider, destination category, expected size,
licence, provenance, external-runtime effect flag, approved effects,
correlation/request IDs, and decision time.

Approval authorizes adding only the reviewed model to the reviewed provider
destination. It does not grant runtime installation, lifecycle management,
update, uninstall, reconfiguration, model deletion, privileged work, future
downloads, or chat. Reusing an external runtime does not transfer ownership.

## State machine and readiness

The durable stage order is:

```text
DraftPlan
  -> AwaitingApproval
  -> Approved
  -> Preparing
  -> CheckingStorage
  -> Acquiring or Registering (verified reuse)
  -> VerifyingRuntime
  -> VerifyingModel
  -> RunningTestInference
  -> Ready
```

`AttentionRequired`, `Failed`, and `Cancelled` are explicit outcomes. Unknown
serialized states are rendered as attention and never as ready.

The core commits `Ready` and model lifecycle `Available` only when the same
transaction has evidence for all of these gates:

1. the exact plan revision was approved;
2. the compatible loopback runtime is healthy and authorized for reuse;
3. the exact provider artifact is locally available;
4. provider registration is verified;
5. the adapter-owned bounded readiness inference succeeds.

A completed pull, provider progress at 100 percent, a process exit, or a model
inventory row alone is not readiness. Incomplete, retained, staged, rejected,
or uncertain artifacts never use lifecycle `Available`.

## Persistence and recovery

Migration `0003_setup_workflow.sql` adds durable setup jobs, immutable approval
scope, bounded retained events, effect outcomes, canonical model metadata, and
provider-artifact mappings. It is additive and runs in the existing SQLite
migration transaction. Model binaries and provider partial layers remain in
provider-owned storage, not the GixGiz database or application staging root.

Each transition uses an expected revision and atomically updates the job,
event sequence, model metadata, and effect evidence that belongs to that
transition. Progress counters must be monotonic and bounded. Event replay uses
an exclusive sequence cursor and bounded pages.

At startup the core classifies interrupted work without silently resuming a
provider mutation:

- plan review and approval states remain pending;
- interrupted checks or verification become retryable attention;
- interrupted acquisition or registration becomes attention with retained or
  uncertain provider effects;
- terminal states remain terminal.

The desktop asks for the latest relevant persisted job after connecting. A UI
disconnect detaches its event subscription; it does not cancel the durable
job. After a stream interruption, Flutter requests an authoritative snapshot
and resumes from the last persisted sequence.

## Storage preflight

Ollama owns its model storage. GixGiz reports the destination as
`ProviderManaged` and never claims that its `staging` or `content` directory
contains the pulled model. When composed, the adapter captures only the
documented default model root or visible `OLLAMA_MODELS` override,
canonicalizes and pins that evidence, and returns a generic safe category label
rather than a private path.

Before acquisition, and again at bounded acquisition checkpoints, the workflow
checks caller-available capacity for that pinned documented/default or visible
override filesystem. The
required capacity is the conservative expected model size plus a fixed safety
margin. A missing destination, failed root validation, or insufficient
capacity pauses the job with an actionable attention state. The workflow does
not enumerate personal files, relocate the provider, or delete unrelated or
provider-owned data. Ollama does not report its effective storage root through
the API, so this preflight remains bounded environment/default-root evidence;
provider acquisition and exact registration checks remain required.

## Acquisition and verification

The Ollama adapter accepts only catalogue-owned canonical model mappings. It
uses the provider's pull stream for acquisition, normalizes numeric progress,
and discards provider status/error text. Lines, buffered fragments, total
bytes, event counts, inactivity, and overall duration are bounded. Closing a
request is the available cooperative cancellation mechanism; it is not treated
as proof that the provider rolled back partial layers.

The overall acquisition deadline is six hours. Ordinary preparation and model
inspection use narrower bounds. The final readiness inference has a 180-second
deadline so CPU-first supported systems can complete real verification without
making every setup check equally long.

After acquisition or verified reuse, the adapter checks the exact local
inventory and provider registration. The final readiness gate sends one fixed,
non-private adapter-owned prompt with fixed bounded generation options. It
accepts only a successful terminal response with bounded non-empty output,
then immediately discards the output. Prompt and output are neither returned
as chat content nor persisted or logged.

The implementation follows Ollama's documented pull, model-list, streaming,
error, and generate APIs. Provider-specific routes and request shapes remain
private to `gixgiz-runtime-ollama`.

## Cancellation, retry, and effects

Cancellation first records `cancellation_requested`, signals the active job
token, and commits a terminal cancelled checkpoint. The terminal cancellation
report classifies each known material effect as:

- `Completed`: verified work completed before cancellation;
- `Retained`: an intentional provider or metadata effect remains;
- `RolledBack`: GixGiz safely reversed an effect it owned;
- `Uncertain`: the provider-side outcome cannot be established safely.

The workflow never claims provider rollback that it cannot verify. It does not
delete provider layers or existing models.

Retry is explicit. It rechecks runtime health, destination availability and
capacity, exact inventory, registration, and inference. It skips acquisition
only when the exact compatible provider artifact is reverified. Incomplete
verification always runs again. If the immutable plan revision is no longer
valid, execution fails closed. The user must cancel the stale job and create
and approve a fresh plan; Task 10 does not revise a plan automatically.

One approved plan permits at most eight retries. This leaves bounded durable
effect-history capacity for truthful terminal and recovery checkpoints. When
the limit is reached, core records a terminal `Failed` snapshot before any new
provider work; prior artifact and effect evidence remains intact, and the user
must create and approve a fresh plan.

## Transport and Flutter

Setup operations use the existing bearer-authenticated, handshake-gated
loopback boundary under `/internal/v1/setup`. Requests require valid matching
correlation/request IDs and bounded JSON bodies. Job events contain complete
provider-neutral persisted snapshots with monotonic sequence numbers; they do
not contain raw provider progress or output.

The Foundation setup panel shows plan review, approval, progress, recovery,
attention, failure, cancellation effects, retry, and verified ready states.
Actions remain keyboard accessible, status changes use semantic live regions,
and layouts support enlarged text. Flutter displays `Ready` only when the Rust
snapshot is ready.

## Deterministic and real-provider tests

Required CI tests use fake providers, temporary SQLite databases, bounded local
loopback servers, and no administrator rights or external network. They cover
approval and denial, initial and mid-acquisition storage loss, existing-model
reuse, bounded and per-layer progress, cancellation before acquisition and
during acquisition/readiness verification, registration and integrity
failures, inference failure and timeout, restart recovery, retry, and the complete `Ready`
evidence gate.

The real Ollama setup smoke test is ignored by default. It requires explicit
environment gates, an explicitly prepared compatible local runtime, enough
storage, and the allowlisted small smoke model. It records only provider
version, provider model identifier, whether acquisition was needed, categorical
storage effects, and verification outcome. It never deletes or updates Ollama,
deletes a model, or persists prompt/output content.

## Known limitations

- The current local catalogue does not contain an independently trusted
  expected model digest. Provider-reported integrity can be recorded and a
  supplied mismatch is rejected, but GixGiz cannot claim independent
  cryptographic identity or quarantine provider-owned files in this task.
- Ollama does not expose an API that proves the running process's effective
  model-storage root, so environment/default-root evidence remains bounded and
  explicit.
- Closing a pull connection does not prove server-side rollback; cancellation
  can therefore retain or leave uncertain provider effects.
- `Ready` here means the model setup passed its readiness gates. Task 11 will
  add streaming chat and conversation persistence; those are not part of this
  workflow.
