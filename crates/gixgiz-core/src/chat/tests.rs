use std::{
    io::{self, Write},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use gixgiz_contracts::{
    CandidateModelId, ChatFailureCode, ChatMessageStatus, ConversationId, CorrelationId,
    GenerationId, RUNTIME_REPORT_SCHEMA_VERSION, RequestId, RuntimeCapabilityAvailability,
    RuntimeCapabilityDescriptor, RuntimeCapabilityKind, RuntimeConsentDecision, RuntimeDisplayName,
    RuntimeEndpointSafety, RuntimeModelInventory, RuntimeModelMappingStatus, RuntimeModelSummary,
    RuntimeProviderId, RuntimeProviderModelId, RuntimeProviderModelMapping, RuntimeState,
};
use gixgiz_persistence::{DataRoot, Persistence};
use gixgiz_runtime::{
    RuntimeCancellationToken, RuntimeError, RuntimeGenerationResult, RuntimeObservation,
    RuntimeOperationContext, testing::FakeRuntimeProvider,
};

use super::*;

const MODEL: &str = "qwen2.5.0.5b-instruct";

const PRIVATE_CHAT_SENTINEL: &str = "GIXGIZ_PRIVATE_CHAT_SENTINEL_DO_NOT_LOG";

#[derive(Clone, Default)]
struct LogBuffer(Arc<Mutex<Vec<u8>>>);

impl Write for LogBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("log buffer lock poisoned"))?
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'writer> tracing_subscriber::fmt::MakeWriter<'writer> for LogBuffer {
    type Writer = Self;

    fn make_writer(&'writer self) -> Self::Writer {
        self.clone()
    }
}

impl LogBuffer {
    fn contents(&self) -> String {
        self.0.lock().map_or_else(
            |_| String::new(),
            |bytes| String::from_utf8_lossy(&bytes).into_owned(),
        )
    }

    fn clear(&self) {
        if let Ok(mut bytes) = self.0.lock() {
            bytes.clear();
        }
    }
}
fn production_log_capture() -> LogBuffer {
    static LOGS: OnceLock<LogBuffer> = OnceLock::new();
    LOGS.get_or_init(|| {
        let logs = LogBuffer::default();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .without_time()
            .with_ansi(false)
            .with_target(false)
            .with_writer(logs.clone())
            .finish();
        tracing::subscriber::set_global_default(subscriber).expect("install test log subscriber");
        logs
    })
    .clone()
}

fn provider_id() -> RuntimeProviderId {
    RuntimeProviderId::new("gixgiz.runtime.test.v1")
}

fn observation() -> RuntimeObservation {
    RuntimeObservation {
        provider_id: provider_id(),
        display_name: RuntimeDisplayName::new("Test runtime"),
        state: RuntimeState::Ready,
        endpoint_safety: RuntimeEndpointSafety::LoopbackVerified,
        version: None,
        // Reuse consent is only accepted when the provider advertises inventory.
        capabilities: vec![RuntimeCapabilityDescriptor {
            kind: RuntimeCapabilityKind::ModelInventory,
            availability: RuntimeCapabilityAvailability::Available,
            reason: None,
        }],
        reasons: Vec::new(),
        warnings: Vec::new(),
    }
}

fn inventory(mapped: bool) -> RuntimeModelInventory {
    RuntimeModelInventory {
        schema_version: RUNTIME_REPORT_SCHEMA_VERSION,
        provider_id: provider_id(),
        models: vec![RuntimeModelSummary {
            provider_model_id: RuntimeProviderModelId::new("qwen2.5:0.5b-instruct"),
            display_name: "Qwen 2.5 Compact".to_owned(),
            size_bytes: Some(1024),
            mapping: RuntimeProviderModelMapping {
                status: if mapped {
                    RuntimeModelMappingStatus::Matched
                } else {
                    RuntimeModelMappingStatus::External
                },
                catalogue_id: mapped.then(|| CandidateModelId::new(MODEL)),
            },
        }],
        truncated: false,
        collected_at_unix_ms: 1,
    }
}

fn fake(
    deltas: Vec<String>,
    result: Result<RuntimeGenerationResult, RuntimeError>,
) -> FakeRuntimeProvider {
    FakeRuntimeProvider::new(
        provider_id(),
        Ok(observation()),
        Ok(observation()),
        Ok(inventory(true)),
    )
    .with_chat_results(deltas, result)
}

fn success() -> Result<RuntimeGenerationResult, RuntimeError> {
    Ok(RuntimeGenerationResult {
        emitted_bytes: 0,
        completed_at_unix_ms: 2,
    })
}

async fn harness(
    temporary: &tempfile::TempDir,
    provider: FakeRuntimeProvider,
    approve_reuse: bool,
) -> (Persistence, ChatService) {
    let root = DataRoot::from_override(temporary.path().join("gixgiz-test-root"))
        .expect("isolated test root initializes");
    let persistence = Persistence::open(root).expect("database opens");
    let provider: Arc<dyn RuntimeProvider> = Arc::new(provider);
    if approve_reuse {
        RuntimeService::with_persistence(provider.clone(), &persistence)
            .set_reuse_consent(
                &provider_id(),
                RuntimeConsentDecision::ApproveReuse,
                RuntimeOperationContext::new(
                    CorrelationId::new(),
                    RequestId::new(),
                    Duration::from_secs(5),
                ),
            )
            .await
            .expect("reuse consent is recorded");
    }
    seed_verified_model(temporary);
    let service = ChatService::with_persistence(provider, &persistence);
    (persistence, service)
}

/// Simulates a completed Task 10 setup by inserting the verified catalogue row.
///
/// Chat may only bind a conversation to a model that setup verified; the
/// conversations foreign key enforces that at the storage layer.
fn seed_verified_model(temporary: &tempfile::TempDir) {
    let database = temporary
        .path()
        .join("gixgiz-test-root")
        .join("data")
        .join("gixgiz.db");
    let connection = rusqlite::Connection::open(database).expect("database opens for seeding");
    connection
        .execute(
            "INSERT OR IGNORE INTO models
             (canonical_model_id, display_name, family, size_class, licence_spdx,
              provenance_url, catalogue_version, expected_size_bytes,
              created_at_unix_ms, updated_at_unix_ms)
             VALUES (?1, 'Qwen 2.5 Compact', 'Qwen 2.5', 'compact', 'Apache-2.0',
                     'https://example.invalid/model', 'catalogue-1', 1024, 1, 1)",
            rusqlite::params![MODEL],
        )
        .expect("verified model row is seeded");
}

fn ids() -> (CorrelationId, RequestId) {
    (CorrelationId::new(), RequestId::new())
}

async fn new_conversation(service: &ChatService) -> ConversationId {
    let (correlation_id, request_id) = ids();
    service
        .create_conversation(CreateConversationRequest {
            title: None,
            correlation_id,
            request_id,
        })
        .await
        .expect("conversation is created")
        .conversation
        .conversation_id
}

/// Waits for the assistant message to reach a durable terminal state.
async fn await_terminal(
    persistence: &Persistence,
    generation_id: GenerationId,
) -> gixgiz_persistence::PersistedChatMessage {
    for _ in 0..200 {
        let message = persistence
            .chat()
            .message_by_generation(generation_id)
            .expect("message reads")
            .expect("assistant message exists");
        if message.status.is_terminal() {
            return message;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("generation did not reach a terminal state");
}

#[tokio::test]
async fn send_streams_ordered_deltas_and_commits_completed() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (persistence, service) = harness(
        &temporary,
        fake(vec!["Hello".to_owned(), ", world".to_owned()], success()),
        true,
    )
    .await;
    let conversation_id = new_conversation(&service).await;
    let (correlation_id, request_id) = ids();

    let sent = service
        .send_message(SendMessageRequest {
            conversation_id,
            content: "hi".to_owned(),
            correlation_id,
            request_id,
        })
        .await
        .expect("generation is admitted");
    let terminal = await_terminal(&persistence, sent.generation_id).await;

    assert_eq!(sent.user_message.content, "hi");
    assert_eq!(sent.assistant_message.status, ChatMessageStatus::Generating);
    assert_eq!(terminal.status, PersistedChatMessageStatus::Completed);
    assert_eq!(terminal.content, "Hello, world");
    assert!(terminal.failure_code.is_none());
}

#[tokio::test]
async fn cancellation_commits_partial_content_and_never_completes() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (persistence, service) = harness(
        &temporary,
        fake(vec!["partial".to_owned()], Err(RuntimeError::Cancelled)),
        true,
    )
    .await;
    let conversation_id = new_conversation(&service).await;
    let (correlation_id, request_id) = ids();

    let sent = service
        .send_message(SendMessageRequest {
            conversation_id,
            content: "hi".to_owned(),
            correlation_id,
            request_id,
        })
        .await
        .expect("generation is admitted");
    let (cancel_correlation, cancel_request) = ids();
    service
        .cancel_generation(CancelGenerationRequest {
            generation_id: sent.generation_id,
            correlation_id: cancel_correlation,
            request_id: cancel_request,
        })
        .await
        .expect("cancellation is accepted");
    let terminal = await_terminal(&persistence, sent.generation_id).await;

    assert_eq!(terminal.status, PersistedChatMessageStatus::Cancelled);
    assert_ne!(terminal.status, PersistedChatMessageStatus::Completed);
    assert!(terminal.cancellation_requested);
}

#[tokio::test]
async fn provider_failure_never_becomes_assistant_text() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (persistence, service) = harness(
        &temporary,
        fake(Vec::new(), Err(RuntimeError::ProviderUnavailable)),
        true,
    )
    .await;
    let conversation_id = new_conversation(&service).await;
    let (correlation_id, request_id) = ids();

    let sent = service
        .send_message(SendMessageRequest {
            conversation_id,
            content: "hi".to_owned(),
            correlation_id,
            request_id,
        })
        .await
        .expect("generation is admitted");
    let terminal = await_terminal(&persistence, sent.generation_id).await;

    assert_eq!(terminal.status, PersistedChatMessageStatus::Failed);
    assert_eq!(
        terminal.failure_code.as_deref(),
        Some("chat.provider_disconnected")
    );
    assert!(terminal.content.is_empty());
}

#[tokio::test]
async fn default_chat_tracing_excludes_private_message_and_provider_content() {
    let logs = production_log_capture();
    logs.clear();
    let cases = vec![
        (vec![PRIVATE_CHAT_SENTINEL.to_owned()], success()),
        (Vec::new(), Err(RuntimeError::ProviderUnavailable)),
        (
            vec![PRIVATE_CHAT_SENTINEL.to_owned()],
            Err(RuntimeError::Cancelled),
        ),
    ];

    for (deltas, result) in cases {
        let temporary = tempfile::tempdir().expect("temporary directory is available");
        let (persistence, service) = harness(&temporary, fake(deltas, result), true).await;
        let conversation_id = new_conversation(&service).await;
        let (correlation_id, request_id) = ids();
        let sent = service
            .send_message(SendMessageRequest {
                conversation_id,
                content: PRIVATE_CHAT_SENTINEL.to_owned(),
                correlation_id,
                request_id,
            })
            .await
            .expect("generation is admitted");
        await_terminal(&persistence, sent.generation_id).await;
    }

    let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
    while !logs
        .contents()
        .contains("chat generation terminal state committed")
    {
        assert!(
            tokio::time::Instant::now() < deadline,
            "terminal log missing"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    let output = logs.contents();
    assert!(output.contains("chat generation terminal state committed"));
    assert!(!output.contains(PRIVATE_CHAT_SENTINEL));
}
#[tokio::test]
async fn missing_reuse_consent_blocks_chat_with_an_actionable_code() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (_persistence, service) =
        harness(&temporary, fake(vec!["x".to_owned()], success()), false).await;
    let conversation_id = new_conversation(&service).await;
    let (correlation_id, request_id) = ids();

    let outcome = service
        .send_message(SendMessageRequest {
            conversation_id,
            content: "hi".to_owned(),
            correlation_id,
            request_id,
        })
        .await;

    assert!(matches!(
        outcome,
        Err(CoreError::Chat(ChatFailureCode::RuntimeConsentRequired))
    ));
}

#[tokio::test]
async fn oversized_message_is_rejected_before_any_provider_call() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let provider = fake(vec!["x".to_owned()], success());
    let (persistence, service) = harness(&temporary, provider, true).await;
    let conversation_id = new_conversation(&service).await;
    let (correlation_id, request_id) = ids();

    let outcome = service
        .send_message(SendMessageRequest {
            conversation_id,
            content: "a".repeat(MAX_USER_MESSAGE_BYTES + 1),
            correlation_id,
            request_id,
        })
        .await;

    assert!(matches!(
        outcome,
        Err(CoreError::Chat(ChatFailureCode::MessageTooLarge))
    ));
    assert!(
        persistence
            .chat()
            .messages(conversation_id)
            .expect("messages read")
            .is_empty()
    );
}

#[tokio::test]
async fn unknown_conversation_fails_closed() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (_persistence, service) =
        harness(&temporary, fake(vec!["x".to_owned()], success()), true).await;
    let (correlation_id, request_id) = ids();

    let outcome = service
        .send_message(SendMessageRequest {
            conversation_id: ConversationId::new(),
            content: "hi".to_owned(),
            correlation_id,
            request_id,
        })
        .await;

    assert!(matches!(
        outcome,
        Err(CoreError::Chat(ChatFailureCode::ConversationNotFound))
    ));
}

#[tokio::test]
async fn history_reloads_after_restart_and_interrupted_work_is_not_completed() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let conversation_id;
    let generation_id = GenerationId::new();
    {
        let (persistence, service) =
            harness(&temporary, fake(vec!["x".to_owned()], success()), true).await;
        conversation_id = new_conversation(&service).await;
        persistence
            .chat()
            .append_user_message(
                conversation_id,
                gixgiz_contracts::MessageId::new(),
                "before restart",
                10,
            )
            .expect("user message persists");
        persistence
            .chat()
            .open_assistant_generation(
                conversation_id,
                gixgiz_contracts::MessageId::new(),
                generation_id,
                11,
            )
            .expect("generation opens");
        persistence
            .chat()
            .checkpoint_assistant_content(generation_id, "half", 1, 12)
            .expect("checkpoint commits");
    }

    let (_persistence, service) =
        harness(&temporary, fake(vec!["x".to_owned()], success()), true).await;
    let recovered = service
        .recover_interrupted()
        .await
        .expect("recovery succeeds");
    let (correlation_id, request_id) = ids();
    let snapshot = service
        .get_conversation(GetConversationRequest {
            conversation_id,
            correlation_id,
            request_id,
        })
        .await
        .expect("conversation reloads")
        .conversation;
    let assistant = snapshot
        .messages
        .iter()
        .find(|message| message.generation_id == Some(generation_id))
        .expect("interrupted message survives restart");

    assert_eq!(recovered, 1);
    assert_eq!(snapshot.messages.len(), 2);
    assert_eq!(assistant.status, ChatMessageStatus::Failed);
    assert_ne!(assistant.status, ChatMessageStatus::Completed);
    assert_eq!(assistant.content, "half");
    assert!(snapshot.active_generation_id.is_none());
    assert!(
        snapshot
            .warnings
            .iter()
            .any(|warning| warning.code == ChatWarningCode::InterruptedGenerationRecovered)
    );
}

#[tokio::test]
async fn a_verified_model_is_bound_and_surfaced_to_the_user() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (persistence, service) =
        harness(&temporary, fake(vec!["ok".to_owned()], success()), true).await;
    let conversation_id = new_conversation(&service).await;
    let (correlation_id, request_id) = ids();

    let sent = service
        .send_message(SendMessageRequest {
            conversation_id,
            content: "hi".to_owned(),
            correlation_id,
            request_id,
        })
        .await
        .expect("generation is admitted");
    await_terminal(&persistence, sent.generation_id).await;
    let (snapshot_correlation, snapshot_request) = ids();
    let snapshot = service
        .get_conversation(GetConversationRequest {
            conversation_id,
            correlation_id: snapshot_correlation,
            request_id: snapshot_request,
        })
        .await
        .expect("conversation reads")
        .conversation;

    let model = snapshot.model.expect("verified model is surfaced");
    assert_eq!(model.canonical_model_id, CandidateModelId::new(MODEL));
    assert_eq!(
        persistence
            .chat()
            .conversation(conversation_id)
            .expect("conversation reads")
            .expect("conversation exists")
            .canonical_model_id
            .as_deref(),
        Some(MODEL)
    );
}

#[tokio::test]
async fn cancelling_an_unknown_generation_is_not_accepted() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (_persistence, service) = harness(&temporary, fake(Vec::new(), success()), true).await;
    let (correlation_id, request_id) = ids();

    let outcome = service
        .cancel_generation(CancelGenerationRequest {
            generation_id: GenerationId::new(),
            correlation_id,
            request_id,
        })
        .await
        .expect("cancellation resolves");

    assert!(!outcome.accepted);
}

#[tokio::test]
async fn observing_an_unknown_generation_fails_closed() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (_persistence, service) = harness(&temporary, fake(Vec::new(), success()), true).await;

    let outcome = service.observe(GenerationId::new(), 0);

    assert!(matches!(
        outcome.err(),
        Some(CoreError::Chat(ChatFailureCode::GenerationNotFound))
    ));
}

#[tokio::test]
async fn titles_are_bounded_and_control_characters_are_neutralized() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (_persistence, service) = harness(&temporary, fake(Vec::new(), success()), true).await;
    let conversation_id = new_conversation(&service).await;
    let (correlation_id, request_id) = ids();

    let renamed = service
        .rename_conversation(RenameConversationRequest {
            conversation_id,
            title: format!("line\u{0}break {}", "t".repeat(200)),
            correlation_id,
            request_id,
        })
        .await
        .expect("conversation is renamed")
        .conversation;

    assert!(renamed.title.len() <= MAX_CONVERSATION_TITLE_BYTES);
    assert!(!renamed.title.chars().any(char::is_control));
}

#[tokio::test]
async fn cancellation_token_reaches_a_registered_generation() {
    let registry = GenerationRegistry::default();
    let generation_id = GenerationId::new();
    let token = RuntimeCancellationToken::new();

    assert!(registry.register(generation_id, token.clone()));
    assert!(registry.cancel(generation_id));
    assert!(token.is_cancelled());
}

#[tokio::test]
async fn active_generation_delete_is_rejected_without_losing_its_cancellation_target() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (persistence, service) = harness(&temporary, fake(Vec::new(), success()), true).await;
    let conversation_id = new_conversation(&service).await;
    let generation_id = GenerationId::new();
    let cancellation = RuntimeCancellationToken::new();
    persistence
        .chat()
        .admit_generation(
            conversation_id,
            MessageId::new(),
            MessageId::new(),
            generation_id,
            "keep this",
            10,
        )
        .expect("generation is admitted");
    assert!(
        service
            .generations
            .register(generation_id, cancellation.clone())
    );

    let (correlation_id, request_id) = ids();
    let rejected = service
        .delete_conversation(DeleteConversationRequest {
            conversation_id,
            correlation_id,
            request_id,
        })
        .await;

    assert!(matches!(
        rejected,
        Err(CoreError::Chat(ChatFailureCode::GenerationAlreadyActive))
    ));
    assert!(service.generations.cancel(generation_id));
    assert!(cancellation.is_cancelled());
    assert_eq!(
        persistence
            .chat()
            .messages(conversation_id)
            .expect("messages remain")
            .len(),
        2
    );

    persistence
        .chat()
        .request_cancellation(generation_id, 11)
        .expect("cancellation intent persists");
    persistence
        .chat()
        .finish_assistant_generation(
            generation_id,
            PersistedGenerationOutcome::Cancelled,
            "",
            None,
            1,
            12,
        )
        .expect("terminal cancellation commits");
    service.generations.release(generation_id);
    let (correlation_id, request_id) = ids();
    let deleted = service
        .delete_conversation(DeleteConversationRequest {
            conversation_id,
            correlation_id,
            request_id,
        })
        .await
        .expect("terminal conversation deletes");
    assert!(deleted.deleted);
    assert_eq!(deleted.deleted_message_count, 2);
}

async fn assert_terminal_write_failure_is_not_published(
    outcome: Result<RuntimeGenerationResult, RuntimeError>,
    cancellation_requested: bool,
) {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let conversation_id;
    let generation_id = GenerationId::new();
    {
        let (persistence, service) = harness(&temporary, fake(Vec::new(), success()), true).await;
        conversation_id = new_conversation(&service).await;
        let admission = persistence
            .chat()
            .admit_generation(
                conversation_id,
                MessageId::new(),
                MessageId::new(),
                generation_id,
                "private user content",
                10,
            )
            .expect("generation is admitted");
        let cancellation = RuntimeCancellationToken::new();
        assert!(
            service
                .generations
                .register(generation_id, cancellation.clone())
        );
        if cancellation_requested {
            cancellation.cancel();
            persistence
                .chat()
                .request_cancellation(generation_id, 11)
                .expect("cancellation intent persists");
        }

        let database = temporary
            .path()
            .join("gixgiz-test-root")
            .join("data")
            .join("gixgiz.db");
        let fault = rusqlite::Connection::open(database).expect("fault connection opens");
        fault
            .execute_batch(
                "CREATE TRIGGER fail_chat_terminal
                 BEFORE UPDATE OF status ON messages
                 WHEN OLD.generation_id IS NOT NULL
                      AND NEW.status IN ('completed', 'cancelled', 'failed')
                 BEGIN
                     SELECT RAISE(ABORT, 'injected terminal persistence failure');
                 END;",
            )
            .expect("terminal fault is installed");

        let (correlation_id, request_id) = ids();
        let task = GenerationTask {
            generation_id,
            conversation_id,
            assistant_message_id: admission.assistant_message.message_id,
            correlation_id,
            request_id,
            canonical_model_id: CandidateModelId::new(MODEL),
            messages: Vec::new(),
            cancellation,
        };
        let mut state = GenerationState::new(&task);
        state.accumulated = "private assistant output".to_owned();
        service.emit(&mut state, ChatGenerationEventKind::Started, None, None);
        service.commit(state, outcome, &task).await;

        let subscription = service
            .observe(generation_id, 0)
            .expect("durability interruption remains observable");
        assert!(
            subscription
                .replay
                .iter()
                .all(|event| event.terminal_state.is_none())
        );
        let interruption = subscription.replay.last().expect("final event exists");
        assert_eq!(
            interruption.kind,
            ChatGenerationEventKind::DurabilityInterrupted
        );
        assert_eq!(
            interruption.error.as_ref().map(|error| error.code.as_str()),
            Some("chat.persistence_unavailable")
        );
        let durable = persistence
            .chat()
            .message_by_generation(generation_id)
            .expect("generation reads")
            .expect("generation remains durable");
        assert_eq!(durable.status, PersistedChatMessageStatus::Generating);

        fault
            .execute_batch("DROP TRIGGER fail_chat_terminal")
            .expect("terminal fault is removed");
    }

    let root = DataRoot::from_override(temporary.path().join("gixgiz-test-root"))
        .expect("isolated test root initializes");
    let persistence = Persistence::open(root).expect("database reopens");
    assert_eq!(
        persistence
            .chat()
            .recover_interrupted_generations(20)
            .expect("restart recovery commits"),
        1
    );
    let recovered = persistence
        .chat()
        .message_by_generation(generation_id)
        .expect("generation reads")
        .expect("generation survives restart");
    assert_eq!(
        recovered.status,
        if cancellation_requested {
            PersistedChatMessageStatus::Cancelled
        } else {
            PersistedChatMessageStatus::Failed
        }
    );
}

#[tokio::test]
async fn provider_completion_is_not_published_when_terminal_storage_fails() {
    assert_terminal_write_failure_is_not_published(
        Ok(RuntimeGenerationResult {
            emitted_bytes: 24,
            completed_at_unix_ms: 12,
        }),
        false,
    )
    .await;
}

#[tokio::test]
async fn provider_cancellation_is_not_published_when_terminal_storage_fails() {
    assert_terminal_write_failure_is_not_published(Err(RuntimeError::Cancelled), true).await;
}

#[tokio::test]
async fn provider_failure_is_not_published_when_terminal_storage_fails() {
    assert_terminal_write_failure_is_not_published(Err(RuntimeError::ProviderUnavailable), false)
        .await;
}

#[tokio::test]
async fn chat_runtime_status_requires_verified_local_runtime_and_model_evidence() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (_persistence, service) = harness(&temporary, fake(Vec::new(), success()), true).await;
    let (correlation_id, request_id) = ids();

    let response = service
        .runtime_status(ChatRuntimeStatusRequest {
            conversation_id: None,
            correlation_id,
            request_id,
        })
        .await
        .expect("chat runtime status resolves");

    assert!(response.status.ready);
    assert_eq!(response.status.locality, ChatLocalityStatus::RunningLocally);
    assert_eq!(
        response
            .status
            .model
            .as_ref()
            .map(|model| model.canonical_model_id.as_str()),
        Some(MODEL)
    );
    assert!(response.status.blocked_by.is_none());
}

#[tokio::test]
async fn chat_runtime_status_reports_runtime_loss_without_locality_claim() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let provider = FakeRuntimeProvider::new(
        provider_id(),
        Err(RuntimeError::ProviderUnavailable),
        Err(RuntimeError::ProviderUnavailable),
        Ok(inventory(true)),
    )
    .with_chat_results(Vec::new(), success());
    let (_persistence, service) = harness(&temporary, provider, false).await;
    let (correlation_id, request_id) = ids();

    let response = service
        .runtime_status(ChatRuntimeStatusRequest {
            conversation_id: None,
            correlation_id,
            request_id,
        })
        .await
        .expect("unavailable status remains a safe report");

    assert!(!response.status.ready);
    assert_eq!(response.status.locality, ChatLocalityStatus::Unknown);
    assert_eq!(
        response.status.blocked_by,
        Some(ChatFailureCode::RuntimeUnavailable)
    );
    assert_eq!(
        response.status.recovery_action,
        Some(ChatRecoveryAction::CheckRuntime)
    );
}

#[tokio::test]
async fn chat_runtime_status_reports_missing_model_without_locality_claim() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let provider = fake(Vec::new(), success());
    let (persistence, _service) = harness(&temporary, provider, true).await;
    let unavailable_provider = FakeRuntimeProvider::new(
        provider_id(),
        Ok(observation()),
        Ok(observation()),
        Ok(inventory(false)),
    )
    .with_chat_results(Vec::new(), success());
    let service = ChatService::with_persistence(Arc::new(unavailable_provider), &persistence);
    let (correlation_id, request_id) = ids();

    let response = service
        .runtime_status(ChatRuntimeStatusRequest {
            conversation_id: None,
            correlation_id,
            request_id,
        })
        .await
        .expect("missing model remains a safe report");

    assert!(!response.status.ready);
    assert_eq!(response.status.locality, ChatLocalityStatus::Unknown);
    assert_eq!(
        response.status.blocked_by,
        Some(ChatFailureCode::ModelUnavailable)
    );
    assert_eq!(
        response.status.recovery_action,
        Some(ChatRecoveryAction::RunModelSetup)
    );
}
