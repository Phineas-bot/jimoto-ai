# Task 11: Implement local streaming chat

- **GitHub issue:** [#12](https://github.com/Phineas-bot/gixgiz/issues/12)
- **Logical task number:** 11; GitHub issue #11 was already used by an earlier pull request
- **Depends on:** Tasks 04, 05, 09 and 10
- **Primary ADRs:** 0001–0005 plus accepted runtime decisions

## Context and user value

After verified setup, GixGiz needs one immediately useful workspace. Local chat proves the complete flow from Flutter through the authenticated core boundary, persisted sessions and the runtime adapter without exposing Ollama to the UI.

## Desired behavior

The user can create a local conversation, send a message, receive tokens incrementally, stop generation, see runtime/model locality and reopen the conversation after restarting GixGiz.

## In scope

- Provider-neutral chat/session contracts.
- Local session creation and normalized user/assistant messages.
- Incremental ordered token/event streaming through the Task 04 transport.
- Stop-generation and cancellation propagation to the Ollama adapter.
- Active runtime/model and local/offline status.
- SQLite conversation/message persistence and reload.
- Basic conversation list, rename/title behavior and explicit deletion.
- Empty, loading, generating, ready, degraded, failed and cancelled UI states.
- Stable user-facing error mapping and correlation IDs.
- Deterministic fake-provider integration tests and a separate real-provider smoke test.

## Out of scope

- Attachments, RAG, tools, agents or filesystem actions.
- Cloud providers, accounts or synchronization.
- Markdown extensions that require unsafe HTML execution.
- Voice, image or multimodal input.
- Advanced context management, branching conversations or public API compatibility.

## Architecture constraints

- Flutter calls only GixGiz contracts; it never calls Ollama directly.
- Provider output is normalized in Rust before crossing the boundary.
- Conversation persistence is owned by the Rust core.
- Streaming events are ordered and correlated to one request/session.
- Cancellation propagates through UI, transport, session service and runtime adapter.
- Provider/runtime loss is an explicit recoverable state, not a fabricated assistant response.

## Security and privacy constraints

- Messages remain local by default.
- Do not log full prompts, responses or conversation content.
- No telemetry or external network transmission.
- Render model output as untrusted content; do not execute HTML, scripts, links or commands automatically.
- Bound message, context, stream, output and session sizes.
- Conversation deletion must clearly describe and remove owned database/content records.

## Acceptance criteria

- [X] AC-1: A user can create, persist, reopen and delete a local conversation.
- [X] AC-2: Assistant output streams incrementally in order.
- [X] AC-3: Stop generation cancels the provider operation and produces a distinct cancelled state.
- [X] AC-4: Flutter never accesses Ollama or SQLite directly.
- [X] AC-5: Runtime/model identity and Local/Offline status are visible without technical overload.
- [X] AC-6: Provider loss, incompatible runtime, unavailable model and timeout map to stable actionable errors.
- [ ] AC-7: Private message content is absent from default logs and telemetry. **PARTIAL** - no log statement carries message content, but no automated redaction test asserts it.
- [X] AC-8: UI supports keyboard use, visible focus, scalable text and semantic streaming/status labels.
- [X] AC-9: Integration tests cover streaming, cancellation, persistence, restart and runtime loss.

## Required tests

- Create/send/stream/complete with deterministic fake provider.
- Cancellation before first token and during streaming.
- Provider timeout, disconnect and malformed event.
- Conversation save, reload, ordering and deletion.
- Application restart with persisted history.
- Runtime/model unavailable or changed.
- Output-size and message-size limits.
- Log-redaction assertion.
- Flutter widget and accessibility states.
- Real Ollama/model smoke test after Task 10 readiness.

## Validation commands

Run all repository Rust, Flutter, contract, migration and Windows build checks. Run real-provider chat smoke tests separately and record exact Ollama/model versions.

## Documentation updates

- Local chat behavior and privacy statement.
- Session/message schema and deletion semantics.
- Supported limitations for v0.1.
- Troubleshooting for runtime/model loss and cancellation.

## Completion evidence

Report fake and real provider evidence, stream/cancellation behavior, persistence schema changes, privacy checks, accessibility checks, exact commands and any context/model limitation.