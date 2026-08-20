use std::{sync::Arc, time::Duration};

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
