use std::sync::{Arc, Barrier};

use gixgiz_contracts::{ConversationId, GenerationId, MessageId, RuntimeProviderId};
use gixgiz_persistence::{
    CURRENT_SCHEMA_VERSION, ChatRepository, DataRoot, INTERRUPTED_FAILURE_CODE,
    PersistedChatMessageStatus, PersistedChatRole, PersistedConversationInput,
    PersistedGenerationOutcome, Persistence, PersistenceError,
};

const NOW: u64 = 1_760_000_000_000;

fn test_root(temporary: &tempfile::TempDir) -> DataRoot {
    DataRoot::from_override(temporary.path().join("gixgiz-test-root"))
        .expect("isolated test root initializes")
}

fn conversation_input(conversation_id: ConversationId) -> PersistedConversationInput {
    PersistedConversationInput {
        conversation_id,
        title: "First conversation".to_owned(),
        canonical_model_id: None,
        runtime_provider_id: RuntimeProviderId::new("gixgiz.runtime.test.v1"),
        created_at_unix_ms: NOW,
    }
}

fn open_repository(temporary: &tempfile::TempDir) -> (Persistence, ChatRepository) {
    let persistence = Persistence::open(test_root(temporary)).expect("database opens");
    let repository = persistence.chat();
    (persistence, repository)
}

#[test]
fn migration_four_applies_and_reports_the_current_schema_version() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let persistence = Persistence::open(test_root(&temporary)).expect("database opens");

    assert_eq!(CURRENT_SCHEMA_VERSION, 4);
    assert_eq!(
        persistence
            .health_check()
            .expect("health check succeeds")
            .schema_version,
        CURRENT_SCHEMA_VERSION
    );
}

#[test]
fn conversation_round_trips_with_ordered_messages() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (_persistence, repository) = open_repository(&temporary);
    let conversation_id = ConversationId::new();
    repository
        .create_conversation(&conversation_input(conversation_id))
        .expect("conversation is created");

    repository
        .append_user_message(conversation_id, MessageId::new(), "first", NOW + 1)
        .expect("first user message persists");
    let generation = GenerationId::new();
    repository
        .open_assistant_generation(conversation_id, MessageId::new(), generation, NOW + 2)
        .expect("assistant generation opens");
    repository
        .finish_assistant_generation(
            generation,
            PersistedGenerationOutcome::Completed,
            "answer",
            None,
            3,
            NOW + 3,
        )
        .expect("assistant generation completes");
    repository
        .append_user_message(conversation_id, MessageId::new(), "second", NOW + 4)
        .expect("second user message persists");

    let messages = repository.messages(conversation_id).expect("messages read");
    let sequences: Vec<u32> = messages.iter().map(|message| message.sequence).collect();
    let roles: Vec<PersistedChatRole> = messages.iter().map(|message| message.role).collect();

    assert_eq!(sequences, vec![1, 2, 3]);
    assert_eq!(
        roles,
        vec![
            PersistedChatRole::User,
            PersistedChatRole::Assistant,
            PersistedChatRole::User
        ]
    );
    assert_eq!(messages[1].status, PersistedChatMessageStatus::Completed);
    assert_eq!(messages[1].content, "answer");
    assert!(messages[1].completed_at_unix_ms.is_some());
}

#[test]
fn history_survives_reopening_the_database() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let conversation_id = ConversationId::new();
    {
        let (_persistence, repository) = open_repository(&temporary);
        repository
            .create_conversation(&conversation_input(conversation_id))
            .expect("conversation is created");
        repository
            .append_user_message(conversation_id, MessageId::new(), "persisted", NOW + 1)
            .expect("user message persists");
    }

    let (_persistence, repository) = open_repository(&temporary);
    let messages = repository.messages(conversation_id).expect("messages read");

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "persisted");
}

#[test]
fn only_one_generation_can_be_active_per_conversation() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (_persistence, repository) = open_repository(&temporary);
    let conversation_id = ConversationId::new();
    repository
        .create_conversation(&conversation_input(conversation_id))
        .expect("conversation is created");
    repository
        .open_assistant_generation(
            conversation_id,
            MessageId::new(),
            GenerationId::new(),
            NOW + 1,
        )
        .expect("first generation opens");

    let conflict = repository.open_assistant_generation(
        conversation_id,
        MessageId::new(),
        GenerationId::new(),
        NOW + 2,
    );

    assert!(matches!(
        conflict,
        Err(PersistenceError::RecordConflict {
            entity: "chat_generation"
        })
    ));
}

#[test]
fn checkpoints_retain_partial_content_and_cancellation_keeps_it_partial() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (_persistence, repository) = open_repository(&temporary);
    let conversation_id = ConversationId::new();
    repository
        .create_conversation(&conversation_input(conversation_id))
        .expect("conversation is created");
    let generation = GenerationId::new();
    repository
        .open_assistant_generation(conversation_id, MessageId::new(), generation, NOW + 1)
        .expect("generation opens");

    repository
        .checkpoint_assistant_content(generation, "partial", 2, NOW + 2)
        .expect("checkpoint commits");
    assert!(
        repository
            .request_cancellation(generation, NOW + 3)
            .expect("cancellation is recorded")
    );
    let cancelled = repository
        .finish_assistant_generation(
            generation,
            PersistedGenerationOutcome::Cancelled,
            "partial",
            None,
            4,
            NOW + 4,
        )
        .expect("cancellation commits");

    assert_eq!(cancelled.status, PersistedChatMessageStatus::Cancelled);
    assert_eq!(cancelled.content, "partial");
    assert!(cancelled.cancellation_requested);
    assert_ne!(cancelled.status, PersistedChatMessageStatus::Completed);
}

#[test]
fn interrupted_generations_never_recover_as_completed() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let conversation_id = ConversationId::new();
    let interrupted = GenerationId::new();
    let cancelling = GenerationId::new();
    let second_conversation = ConversationId::new();
    {
        let (_persistence, repository) = open_repository(&temporary);
        repository
            .create_conversation(&conversation_input(conversation_id))
            .expect("conversation is created");
        repository
            .create_conversation(&PersistedConversationInput {
                conversation_id: second_conversation,
                ..conversation_input(second_conversation)
            })
            .expect("second conversation is created");
        repository
            .open_assistant_generation(conversation_id, MessageId::new(), interrupted, NOW + 1)
            .expect("interrupted generation opens");
        repository
            .checkpoint_assistant_content(interrupted, "half a sentence", 1, NOW + 2)
            .expect("checkpoint commits");
        repository
            .open_assistant_generation(second_conversation, MessageId::new(), cancelling, NOW + 3)
            .expect("cancelling generation opens");
        repository
            .request_cancellation(cancelling, NOW + 4)
            .expect("cancellation is recorded");
    }

    let (_persistence, repository) = open_repository(&temporary);
    let recovered = repository
        .recover_interrupted_generations(NOW + 10)
        .expect("recovery runs");

    let failed = repository
        .message_by_generation(interrupted)
        .expect("interrupted message reads")
        .expect("interrupted message exists");
    let cancelled = repository
        .message_by_generation(cancelling)
        .expect("cancelling message reads")
        .expect("cancelling message exists");

    assert_eq!(recovered, 2);
    assert_eq!(failed.status, PersistedChatMessageStatus::Failed);
    assert_eq!(
        failed.failure_code.as_deref(),
        Some(INTERRUPTED_FAILURE_CODE)
    );
    assert_eq!(failed.content, "half a sentence");
    assert_eq!(cancelled.status, PersistedChatMessageStatus::Cancelled);
    assert!(cancelled.failure_code.is_none());
}

#[test]
fn deleting_a_conversation_removes_only_its_own_messages() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (_persistence, repository) = open_repository(&temporary);
    let kept = ConversationId::new();
    let removed = ConversationId::new();
    for conversation_id in [kept, removed] {
        repository
            .create_conversation(&PersistedConversationInput {
                conversation_id,
                ..conversation_input(conversation_id)
            })
            .expect("conversation is created");
        repository
            .append_user_message(conversation_id, MessageId::new(), "hello", NOW + 1)
            .expect("user message persists");
    }

    let deleted_messages = repository
        .delete_conversation(removed)
        .expect("conversation is deleted");

    assert_eq!(deleted_messages, 1);
    assert!(
        repository
            .conversation(removed)
            .expect("removed conversation reads")
            .is_none()
    );
    assert!(
        repository
            .messages(removed)
            .expect("removed messages read")
            .is_empty()
    );
    assert_eq!(
        repository.messages(kept).expect("kept messages read").len(),
        1
    );
}

#[test]
fn renaming_updates_only_the_title_and_missing_records_fail_closed() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (_persistence, repository) = open_repository(&temporary);
    let conversation_id = ConversationId::new();
    repository
        .create_conversation(&conversation_input(conversation_id))
        .expect("conversation is created");

    let renamed = repository
        .rename_conversation(conversation_id, "Renamed thread", NOW + 5)
        .expect("conversation is renamed");
    let missing = repository.rename_conversation(ConversationId::new(), "Ghost", NOW + 6);

    assert_eq!(renamed.title, "Renamed thread");
    assert!(matches!(
        missing,
        Err(PersistenceError::RecordNotFound {
            entity: "conversation"
        })
    ));
}

#[test]
fn oversized_and_control_content_is_rejected_before_storage() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (_persistence, repository) = open_repository(&temporary);
    let conversation_id = ConversationId::new();
    repository
        .create_conversation(&conversation_input(conversation_id))
        .expect("conversation is created");

    let oversized = "a".repeat(64 * 1024 + 1);
    let oversized_result =
        repository.append_user_message(conversation_id, MessageId::new(), &oversized, NOW + 1);
    let control_result =
        repository.append_user_message(conversation_id, MessageId::new(), "bad\u{0}text", NOW + 2);
    let empty_result =
        repository.append_user_message(conversation_id, MessageId::new(), "   ", NOW + 3);
    let long_title = "t".repeat(121);
    let title_result = repository.rename_conversation(conversation_id, &long_title, NOW + 4);

    for outcome in [
        oversized_result,
        control_result,
        empty_result,
        title_result.map(|_| unreachable!()),
    ] {
        assert!(matches!(
            outcome,
            Err(PersistenceError::InvalidRecord { .. })
        ));
    }
    assert!(
        repository
            .messages(conversation_id)
            .expect("messages read")
            .is_empty()
    );
}

#[test]
fn newlines_and_tabs_remain_valid_message_content() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (_persistence, repository) = open_repository(&temporary);
    let conversation_id = ConversationId::new();
    repository
        .create_conversation(&conversation_input(conversation_id))
        .expect("conversation is created");

    let message = repository
        .append_user_message(
            conversation_id,
            MessageId::new(),
            "line one\nline two\tindented",
            NOW + 1,
        )
        .expect("multi-line content persists");

    assert_eq!(message.content, "line one\nline two\tindented");
}

#[test]
fn listing_is_bounded_and_reports_truncation() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (_persistence, repository) = open_repository(&temporary);
    for index in 0..3_u64 {
        let conversation_id = ConversationId::new();
        repository
            .create_conversation(&PersistedConversationInput {
                conversation_id,
                created_at_unix_ms: NOW + index,
                ..conversation_input(conversation_id)
            })
            .expect("conversation is created");
    }

    let (page, truncated) = repository.list_conversations(2).expect("page reads");
    let (all, complete) = repository.list_conversations(10).expect("full list reads");

    assert_eq!(page.len(), 2);
    assert!(truncated);
    assert_eq!(all.len(), 3);
    assert!(!complete);
    assert!(matches!(
        repository.list_conversations(0),
        Err(PersistenceError::InvalidRecord { field: "limit" })
    ));
}

#[test]
fn chat_storage_holds_no_provider_payload_or_secret_columns() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (persistence, _repository) = open_repository(&temporary);

    let schema = persistence
        .health_check()
        .expect("health check succeeds")
        .schema_version;
    let columns =
        rusqlite::Connection::open(temporary.path().join("gixgiz-test-root/data/gixgiz.db"))
            .expect("database opens for inspection")
            .prepare("SELECT name FROM pragma_table_info('messages')")
            .expect("column query prepares")
            .query_map([], |row| row.get::<_, String>(0))
            .expect("columns enumerate")
            .collect::<Result<Vec<_>, _>>()
            .expect("columns decode");

    assert_eq!(schema, CURRENT_SCHEMA_VERSION);
    for forbidden in [
        "provider_payload",
        "provider_response",
        "raw_response",
        "prompt",
        "token",
        "secret",
        "log",
        "path",
    ] {
        assert!(
            !columns.iter().any(|column| column.contains(forbidden)),
            "messages exposes a {forbidden} column"
        );
    }
}
#[test]
fn generation_admission_is_atomic_under_concurrent_sends() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (_persistence, repository) = open_repository(&temporary);
    let conversation_id = ConversationId::new();
    repository
        .create_conversation(&conversation_input(conversation_id))
        .expect("conversation is created");
    let repository = Arc::new(repository);
    let barrier = Arc::new(Barrier::new(3));

    let handles: Vec<_> = ["first", "second"]
        .into_iter()
        .map(|content| {
            let repository = repository.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                repository.admit_generation(
                    conversation_id,
                    MessageId::new(),
                    MessageId::new(),
                    GenerationId::new(),
                    content,
                    NOW + 1,
                )
            })
        })
        .collect();
    barrier.wait();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("admission thread joins"))
        .collect();

    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(
                result,
                Err(PersistenceError::RecordConflict {
                    entity: "chat_generation"
                })
            ))
            .count(),
        1
    );
    let messages = repository.messages(conversation_id).expect("messages read");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, PersistedChatRole::User);
    assert_eq!(messages[0].sequence, 1);
    assert_eq!(messages[1].role, PersistedChatRole::Assistant);
    assert_eq!(messages[1].sequence, 2);
    assert_eq!(
        messages
            .iter()
            .filter(|message| message.status == PersistedChatMessageStatus::Generating)
            .count(),
        1
    );
}

#[test]
fn generation_admission_is_independent_across_conversations() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (_persistence, repository) = open_repository(&temporary);
    let conversations = [ConversationId::new(), ConversationId::new()];
    for conversation_id in conversations {
        repository
            .create_conversation(&conversation_input(conversation_id))
            .expect("conversation is created");
        repository
            .admit_generation(
                conversation_id,
                MessageId::new(),
                MessageId::new(),
                GenerationId::new(),
                "hello",
                NOW + 1,
            )
            .expect("conversation admits its own generation");
    }

    for conversation_id in conversations {
        assert_eq!(
            repository
                .messages(conversation_id)
                .expect("messages read")
                .len(),
            2
        );
    }
}

#[test]
fn deletion_rejects_active_generation_then_succeeds_after_terminal_commit() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let (_persistence, repository) = open_repository(&temporary);
    let conversation_id = ConversationId::new();
    let generation_id = GenerationId::new();
    repository
        .create_conversation(&conversation_input(conversation_id))
        .expect("conversation is created");
    repository
        .admit_generation(
            conversation_id,
            MessageId::new(),
            MessageId::new(),
            generation_id,
            "keep this",
            NOW + 1,
        )
        .expect("generation is admitted");

    let rejected = repository.delete_conversation(conversation_id);
    assert!(matches!(
        rejected,
        Err(PersistenceError::RecordConflict {
            entity: "chat_generation"
        })
    ));
    assert!(
        repository
            .conversation(conversation_id)
            .expect("conversation reads")
            .is_some()
    );
    assert_eq!(
        repository
            .messages(conversation_id)
            .expect("messages remain")
            .len(),
        2
    );

    repository
        .finish_assistant_generation(
            generation_id,
            PersistedGenerationOutcome::Cancelled,
            "",
            None,
            1,
            NOW + 2,
        )
        .expect("terminal cancellation commits");
    assert_eq!(
        repository
            .delete_conversation(conversation_id)
            .expect("terminal conversation deletes"),
        2
    );
}
