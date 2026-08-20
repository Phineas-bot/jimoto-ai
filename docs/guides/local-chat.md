# Local chat development guide

## Ownership and trust boundaries

Task 11 adds one local conversation workspace on top of the Task 10 verified
model. Ownership follows the established direction:

```text
Flutter -> CoreClient -> authenticated sidecar -> gixgiz-core (chat)
                                              -> gixgiz-runtime
                                                       |
                                              gixgiz-runtime-ollama
```

`gixgiz-contracts` owns provider-neutral conversation, message, and generation
contracts. `gixgiz-persistence` owns every conversation row. `gixgiz-core` owns
session lifecycle, context assembly, the readiness gate, generation
orchestration, and cancellation. `gixgiz-runtime` owns the provider-neutral
streaming trait. `gixgiz-runtime-ollama` alone knows the chat route, the request
shape, and the response framing.

Flutter never calls a provider, never opens SQLite, and never infers completion.

## Provider boundary

Chat uses the provider chat route, which accepts role-tagged messages and
streams newline-delimited chunks. Task 10's readiness inference keeps its own
separate generation route and is unchanged.

The adapter consumes assistant `message.content` only. Thinking, tool-call, and
image fields are decoded and discarded; they never cross the adapter boundary
and are out of scope for v0.1. Requests disable provider-side thinking.

No provider tag crosses the provider-neutral boundary. The runtime request
carries a canonical catalogue identity, and the adapter resolves it through its
own allowlist. An unmapped identity fails closed rather than guessing a tag.

## Persistence and recovery

Schema version 4 adds two tables:

| Table | Contents |
|---|---|
| `conversations` | title, bound canonical model, provider, sequence allocator, timestamps |
| `messages` | role, status, ordered sequence, bounded content, generation identity, timestamps |

Invariants are enforced in SQL rather than trusted to application code:

- A partial unique index permits **one active generation per conversation**.
- A user message is always `completed` and never carries a generation.
- Terminal status and `completed_at_unix_ms` imply each other in both directions.
- Only a `failed` message may carry a failure code.
- Deleting a conversation cascades to the messages it owns and nothing else.

Assistant output is committed at bounded checkpoints during generation, so a
crash leaves a coherent partial rather than a torn write or an empty message.

On startup the core reclassifies every message still marked `generating`:
cancellation-requested becomes `cancelled`, everything else becomes `failed`
with `chat.generation_interrupted`. **A previously generating message can never
recover as `completed`**, because no terminal provider evidence was observed.

## Readiness gate

Before any generation the core verifies the runtime reports `Ready`, that reuse
consent permits provider access, and that the conversation's model is still in
the provider inventory. The first send binds the verified canonical model to the
conversation; later sends compare against that exact binding and return
`chat.model_changed` on mismatch.

A conversation can only bind a model the setup workflow verified. The
`conversations` foreign key to `models` enforces that at the storage layer, and
an unverified model is reported as `chat.model_unavailable` with a
run-model-setup recovery action.

Chat never selects a different model, pulls a model, installs a runtime, or
restarts a runtime.

## Context construction

Context is assembled only from the selected conversation's persisted messages.
No file is read, no retrieval is performed, and no model summarizes history.

Assembly walks newest-first, admits only `completed` messages, stops at the
message or byte bound, then restores chronological order. Cancelled and failed
assistant messages hold partial text and are deliberately excluded, because
replaying half a sentence as history would corrupt later turns. When anything is
dropped the response carries a `context_truncated` warning.

## Bounds

| Bound | v0.1 value |
|---|---|
| User message | 16 KiB |
| Assistant output | 64 KiB |
| Context messages | 20 |
| Context bytes | 48 KiB |
| Conversation title | 120 bytes |
| Conversations per page | 50 |
| Retained replay events | 64 |
| Generation inactivity | 30 seconds |
| Total generation | 300 seconds |

## Transport

Eight authenticated routes sit behind the existing sidecar guards: bearer token,
completed handshake, valid correlation and request identifiers, JSON content
type, bounded request bodies, concurrency limits, and bounded event streams.

```text
POST /internal/v1/chat/conversations
POST /internal/v1/chat/conversations/list
POST /internal/v1/chat/conversations/{conversation_id}
POST /internal/v1/chat/conversations/{conversation_id}/rename
POST /internal/v1/chat/conversations/{conversation_id}/delete
POST /internal/v1/chat/conversations/{conversation_id}/messages
GET  /internal/v1/chat/generations/{generation_id}/events?after_sequence=
POST /internal/v1/chat/generations/{generation_id}/cancel
```

Sending returns as soon as the generation is admitted; output arrives on the
event stream. Waiting for completion inside the request is impossible under the
sidecar's request deadline and would hide progress from the user.

Events carry a strictly increasing sequence, the generation, conversation and
assistant message identities, and a correlation identifier. Replay is a bounded
in-memory window, not a durable log. When `replay_incomplete` is set the client
must reload the authoritative conversation snapshot rather than reconstruct
state from events. A client disconnect never fails a generation.

## Cancellation

```text
Stop -> POST cancel -> durable cancellation intent -> core cancellation token
     -> runtime operation context -> provider request closed
```

Cancellation is checked when the terminal state is committed, so a generation
that was cancelled can never land as `completed` even if the provider returned
success in the same moment. Retained partial text is committed with a
`cancelled` status and must be presented as incomplete.

Closing a provider connection does not prove the provider stopped generating.
Chat reports what it observed rather than claiming a clean stop.

## Deletion semantics

Explicit deletion removes the conversation row and, by cascade, the messages it
owns. It removes nothing else: not the runtime, not the model, not
provider-owned model files, not setup records, and not other conversations.

## Privacy and logging

Default logs contain identifiers, counts, byte totals, durations, terminal
states, and safe error codes. They never contain user messages, assistant
output, assembled context, or provider payloads. No telemetry is introduced and
nothing leaves the device.

## Safe rendering

Assistant output is untrusted text. It is stored and returned as plain text;
no HTML is executed, no script runs, no link opens automatically, and no fenced
block is executable.

## Deterministic and real-provider tests

Deterministic tests use a fake provider, temporary SQLite databases, and no
network or administrator rights. They cover streaming and completion,
cancellation, provider failure, missing reuse consent, oversized input, unknown
conversations, restart recovery, model binding, context bounding, and bounded
event replay.

The real-provider chat smoke remains ignored by default and requires an
explicitly prepared runtime with a model that setup already verified. It sends
one fixed non-private prompt and records only lengths and identifiers.

## Known limitations

- Task 11 has no attachments, retrieval, tools, agents, branching, or
  multimodal input; those remain out of scope.
- Event replay is in-memory only. After a restart, clients reload the
  authoritative snapshot; cross-restart event replay does not exist.
- Cancellation cannot prove the provider stopped generating server-side.
- Context selection is deterministic and byte-bounded rather than
  token-accurate; no tokenizer is used.
