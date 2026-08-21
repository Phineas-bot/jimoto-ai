//! Durable conversation and message records owned by the Rust core.
//!
//! This module stores bounded conversation text only. It never stores provider
//! payloads, provider logs, secrets, or transport tokens.

use gixgiz_contracts::{ConversationId, GenerationId, MessageId, RuntimeProviderId};
use rusqlite::{OptionalExtension, Row, params};

use crate::{Persistence, PersistenceError};

const TITLE_BYTES_MAX: usize = 120;
const CONTENT_BYTES_MAX: usize = 64 * 1024;
const PROVIDER_ID_BYTES_MAX: usize = 128;
const FAILURE_CODE_BYTES_MAX: usize = 128;
const MODEL_ID_BYTES_MAX: usize = 128;

/// Stable failure code recorded when a generation is interrupted by shutdown.
pub const INTERRUPTED_FAILURE_CODE: &str = "chat.generation_interrupted";

/// Author of one persisted conversation message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PersistedChatRole {
    /// Content written by the person using GixGiz.
    User,
    /// Content produced by the selected local model.
    Assistant,
}

impl PersistedChatRole {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, PersistenceError> {
        match value {
            "user" => Ok(Self::User),
            "assistant" => Ok(Self::Assistant),
            _ => Err(PersistenceError::InvalidRecord { field: "role" }),
        }
    }
}

/// Durable lifecycle state of one persisted message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PersistedChatMessageStatus {
    /// Persisted without an active generation.
    Pending,
    /// A bounded generation is currently producing content.
    Generating,
    /// The message reached a complete terminal state.
    Completed,
    /// Cancellation stopped generation and retained partial content.
    Cancelled,
    /// Generation failed safely and retained partial or empty content.
    Failed,
}

impl PersistedChatMessageStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Generating => "generating",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, PersistenceError> {
        match value {
            "pending" => Ok(Self::Pending),
            "generating" => Ok(Self::Generating),
            "completed" => Ok(Self::Completed),
            "cancelled" => Ok(Self::Cancelled),
            "failed" => Ok(Self::Failed),
            _ => Err(PersistenceError::InvalidRecord { field: "status" }),
        }
    }

    /// Reports whether no further content can be appended.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed)
    }
}

/// One durable conversation record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedConversation {
    /// Durable conversation identity.
    pub conversation_id: ConversationId,
    /// Bounded conversation title.
    pub title: String,
    /// Canonical model bound to the conversation when one is verified.
    pub canonical_model_id: Option<String>,
    /// Bounded display name of the bound model, when the catalogue knows it.
    pub model_display_name: Option<String>,
    /// Bounded family label of the bound model, when the catalogue knows it.
    pub model_family: Option<String>,
    /// Runtime provider serving the conversation.
    pub runtime_provider_id: RuntimeProviderId,
    /// Number of persisted messages.
    pub message_count: u32,
    /// UTC Unix timestamp in milliseconds when the conversation was created.
    pub created_at_unix_ms: u64,
    /// UTC Unix timestamp in milliseconds when the conversation last changed.
    pub updated_at_unix_ms: u64,
}

/// One durable conversation message record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedChatMessage {
    /// Durable message identity.
    pub message_id: MessageId,
    /// Owning conversation.
    pub conversation_id: ConversationId,
    /// Message author.
    pub role: PersistedChatRole,
    /// Durable lifecycle state.
    pub status: PersistedChatMessageStatus,
    /// Deterministic order within the conversation, starting at one.
    pub sequence: u32,
    /// Bounded message text; partial for a non-completed assistant message.
    pub content: String,
    /// Generation attempt that produced an assistant message.
    pub generation_id: Option<GenerationId>,
    /// Whether cooperative cancellation was requested.
    pub cancellation_requested: bool,
    /// Highest emitted transport event sequence for this generation.
    pub last_event_sequence: u64,
    /// Stable failure code recorded for a failed message.
    pub failure_code: Option<String>,
    /// UTC Unix timestamp in milliseconds when the message was created.
    pub created_at_unix_ms: u64,
    /// UTC Unix timestamp in milliseconds when the message last changed.
    pub updated_at_unix_ms: u64,
    /// UTC Unix timestamp in milliseconds when a terminal state was committed.
    pub completed_at_unix_ms: Option<u64>,
}

/// Inputs required to create one durable conversation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedConversationInput {
    /// Durable conversation identity.
    pub conversation_id: ConversationId,
    /// Bounded conversation title.
    pub title: String,
    /// Canonical model bound to the conversation when one is verified.
    pub canonical_model_id: Option<String>,
    /// Runtime provider serving the conversation.
    pub runtime_provider_id: RuntimeProviderId,
    /// UTC Unix timestamp in milliseconds assigned by Rust.
    pub created_at_unix_ms: u64,
}

/// Atomically admitted user and assistant messages for one generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedGenerationAdmission {
    /// Completed user message that initiated the generation.
    pub user_message: PersistedChatMessage,
    /// Assistant message opened in the generating state.
    pub assistant_message: PersistedChatMessage,
}

/// Terminal outcome committed for one assistant generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PersistedGenerationOutcome {
    /// The provider produced a valid terminal completion.
    Completed,
    /// Cooperative cancellation stopped the generation.
    Cancelled,
    /// The generation failed safely.
    Failed,
}

impl PersistedGenerationOutcome {
    const fn status(self) -> PersistedChatMessageStatus {
        match self {
            Self::Completed => PersistedChatMessageStatus::Completed,
            Self::Cancelled => PersistedChatMessageStatus::Cancelled,
            Self::Failed => PersistedChatMessageStatus::Failed,
        }
    }
}

const MESSAGE_COLUMNS: &str = "message_id, conversation_id, role, status, sequence, content, \
     generation_id, cancellation_requested, last_event_sequence, failure_code, \
     created_at_unix_ms, updated_at_unix_ms, completed_at_unix_ms";

/// Typed repository owning every conversation and message write.
#[derive(Clone)]
pub struct ChatRepository {
    persistence: Persistence,
}

impl ChatRepository {
    pub(crate) fn new(persistence: Persistence) -> Self {
        Self { persistence }
    }

    /// Creates one empty conversation.
    pub fn create_conversation(
        &self,
        input: &PersistedConversationInput,
    ) -> Result<PersistedConversation, PersistenceError> {
        validate_title(&input.title)?;
        validate_text(
            "runtime_provider_id",
            input.runtime_provider_id.as_str(),
            PROVIDER_ID_BYTES_MAX,
        )?;
        if let Some(model) = input.canonical_model_id.as_deref() {
            validate_text("canonical_model_id", model, MODEL_ID_BYTES_MAX)?;
        }
        let created = to_i64(input.created_at_unix_ms, "created_at_unix_ms")?;

        self.persistence.with_write_transaction(|transaction| {
            transaction
                .execute(
                    "INSERT INTO conversations
                     (conversation_id, title, canonical_model_id, runtime_provider_id,
                      next_sequence, created_at_unix_ms, updated_at_unix_ms)
                     VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)",
                    params![
                        input.conversation_id.to_string(),
                        input.title,
                        input.canonical_model_id,
                        input.runtime_provider_id.as_str(),
                        created,
                    ],
                )
                .map_err(|source| PersistenceError::sqlite("insert_conversation", source))?;
            Ok(())
        })?;

        self.conversation(input.conversation_id)?
            .ok_or(PersistenceError::RecordNotFound {
                entity: "conversation",
            })
    }

    /// Returns one conversation without its messages.
    pub fn conversation(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Option<PersistedConversation>, PersistenceError> {
        self.persistence.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT c.conversation_id, c.title, c.canonical_model_id,
                            c.runtime_provider_id, c.created_at_unix_ms, c.updated_at_unix_ms,
                            (SELECT COUNT(*) FROM messages m
                             WHERE m.conversation_id = c.conversation_id),
                            cat.display_name, cat.family
                     FROM conversations c
                     LEFT JOIN models cat
                            ON cat.canonical_model_id = c.canonical_model_id
                     WHERE c.conversation_id = ?1",
                    params![conversation_id.to_string()],
                    decode_conversation,
                )
                .optional()
                .map_err(|source| PersistenceError::sqlite("read_conversation", source))?
                .transpose()
        })
    }

    /// Returns a bounded conversation page ordered by most recent activity.
    ///
    /// The returned flag reports whether more conversations exist beyond the page.
    pub fn list_conversations(
        &self,
        limit: u16,
    ) -> Result<(Vec<PersistedConversation>, bool), PersistenceError> {
        if limit == 0 {
            return Err(PersistenceError::InvalidRecord { field: "limit" });
        }
        let probe = i64::from(limit) + 1;
        self.persistence.with_connection(|connection| {
            let mut statement = connection
                .prepare(
                    "SELECT c.conversation_id, c.title, c.canonical_model_id,
                            c.runtime_provider_id, c.created_at_unix_ms, c.updated_at_unix_ms,
                            (SELECT COUNT(*) FROM messages m
                             WHERE m.conversation_id = c.conversation_id),
                            cat.display_name, cat.family
                     FROM conversations c
                     LEFT JOIN models cat
                            ON cat.canonical_model_id = c.canonical_model_id
                     ORDER BY c.updated_at_unix_ms DESC, c.conversation_id
                     LIMIT ?1",
                )
                .map_err(|source| PersistenceError::sqlite("prepare_list_conversations", source))?;
            let rows = statement
                .query_map(params![probe], decode_conversation)
                .map_err(|source| PersistenceError::sqlite("list_conversations", source))?;
            let mut conversations = Vec::new();
            for row in rows {
                let row =
                    row.map_err(|source| PersistenceError::sqlite("decode_conversation", source))?;
                conversations.push(row?);
            }
            let truncated = conversations.len() > usize::from(limit);
            conversations.truncate(usize::from(limit));
            Ok((conversations, truncated))
        })
    }

    /// Returns every message for one conversation in deterministic order.
    pub fn messages(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Vec<PersistedChatMessage>, PersistenceError> {
        self.persistence.with_connection(|connection| {
            let mut statement = connection
                .prepare(&format!(
                    "SELECT {MESSAGE_COLUMNS} FROM messages
                     WHERE conversation_id = ?1 ORDER BY sequence"
                ))
                .map_err(|source| PersistenceError::sqlite("prepare_read_messages", source))?;
            let rows = statement
                .query_map(params![conversation_id.to_string()], decode_message)
                .map_err(|source| PersistenceError::sqlite("read_messages", source))?;
            let mut messages = Vec::new();
            for row in rows {
                let row =
                    row.map_err(|source| PersistenceError::sqlite("decode_message", source))?;
                messages.push(row?);
            }
            Ok(messages)
        })
    }

    /// Replaces one conversation title.
    pub fn rename_conversation(
        &self,
        conversation_id: ConversationId,
        title: &str,
        now_unix_ms: u64,
    ) -> Result<PersistedConversation, PersistenceError> {
        validate_title(title)?;
        let now = to_i64(now_unix_ms, "updated_at_unix_ms")?;
        self.persistence.with_write_transaction(|transaction| {
            let changed = transaction
                .execute(
                    "UPDATE conversations
                     SET title = ?2, updated_at_unix_ms = MAX(?3, updated_at_unix_ms)
                     WHERE conversation_id = ?1",
                    params![conversation_id.to_string(), title, now],
                )
                .map_err(|source| PersistenceError::sqlite("rename_conversation", source))?;
            if changed == 0 {
                return Err(PersistenceError::RecordNotFound {
                    entity: "conversation",
                });
            }
            Ok(())
        })?;

        self.conversation(conversation_id)?
            .ok_or(PersistenceError::RecordNotFound {
                entity: "conversation",
            })
    }

    /// Deletes one conversation and the messages it owns.
    ///
    /// This removes GixGiz-owned rows only. Provider installations, model
    /// artifacts, setup records, and other conversations are never touched.
    pub fn delete_conversation(
        &self,
        conversation_id: ConversationId,
    ) -> Result<u32, PersistenceError> {
        self.persistence.with_write_transaction(|transaction| {
            let active: i64 = transaction
                .query_row(
                    "SELECT COUNT(*) FROM messages
                     WHERE conversation_id = ?1 AND status = 'generating'",
                    params![conversation_id.to_string()],
                    |row| row.get(0),
                )
                .map_err(|source| PersistenceError::sqlite("check_delete_generation", source))?;
            if active > 0 {
                return Err(PersistenceError::RecordConflict {
                    entity: "chat_generation",
                });
            }
            let owned: i64 = transaction
                .query_row(
                    "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
                    params![conversation_id.to_string()],
                    |row| row.get(0),
                )
                .map_err(|source| PersistenceError::sqlite("count_owned_messages", source))?;
            let removed = transaction
                .execute(
                    "DELETE FROM conversations WHERE conversation_id = ?1",
                    params![conversation_id.to_string()],
                )
                .map_err(|source| PersistenceError::sqlite("delete_conversation", source))?;
            if removed == 0 {
                return Err(PersistenceError::RecordNotFound {
                    entity: "conversation",
                });
            }
            u32::try_from(owned).map_err(|_| PersistenceError::InvalidRecord {
                field: "message_count",
            })
        })
    }

    /// Binds one verified canonical model to a conversation.
    ///
    /// Binding is durable so a later send can detect that the model changed
    /// rather than silently generating with a different one.
    pub fn bind_model(
        &self,
        conversation_id: ConversationId,
        canonical_model_id: &str,
        now_unix_ms: u64,
    ) -> Result<(), PersistenceError> {
        validate_text("canonical_model_id", canonical_model_id, MODEL_ID_BYTES_MAX)?;
        let now = to_i64(now_unix_ms, "updated_at_unix_ms")?;
        self.persistence.with_write_transaction(|transaction| {
            let changed = transaction
                .execute(
                    "UPDATE conversations
                     SET canonical_model_id = ?2,
                         updated_at_unix_ms = MAX(?3, updated_at_unix_ms)
                     WHERE conversation_id = ?1",
                    params![conversation_id.to_string(), canonical_model_id, now],
                )
                .map_err(|source| PersistenceError::sqlite("bind_conversation_model", source))?;
            if changed == 0 {
                return Err(PersistenceError::RecordNotFound {
                    entity: "conversation",
                });
            }
            Ok(())
        })
    }

    /// Appends one completed user message and returns it.
    pub fn append_user_message(
        &self,
        conversation_id: ConversationId,
        message_id: MessageId,
        content: &str,
        now_unix_ms: u64,
    ) -> Result<PersistedChatMessage, PersistenceError> {
        validate_content(content)?;
        if content.trim().is_empty() {
            return Err(PersistenceError::InvalidRecord { field: "content" });
        }
        let now = to_i64(now_unix_ms, "created_at_unix_ms")?;
        self.persistence.with_write_transaction(|transaction| {
            let sequence = allocate_sequence(transaction, conversation_id, now)?;
            transaction
                .execute(
                    "INSERT INTO messages
                     (message_id, conversation_id, role, status, sequence, content,
                      generation_id, cancellation_requested, last_event_sequence, failure_code,
                      created_at_unix_ms, updated_at_unix_ms, completed_at_unix_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, 0, 0, NULL, ?7, ?7, ?7)",
                    params![
                        message_id.to_string(),
                        conversation_id.to_string(),
                        PersistedChatRole::User.as_str(),
                        PersistedChatMessageStatus::Completed.as_str(),
                        sequence,
                        content,
                        now,
                    ],
                )
                .map_err(|source| PersistenceError::sqlite("insert_user_message", source))?;
            read_message(transaction, message_id)
        })
    }

    /// Opens one assistant message in the generating state.
    ///
    /// The unique partial index on active generations rejects a second
    /// concurrent generation for the same conversation.
    pub fn open_assistant_generation(
        &self,
        conversation_id: ConversationId,
        message_id: MessageId,
        generation_id: GenerationId,
        now_unix_ms: u64,
    ) -> Result<PersistedChatMessage, PersistenceError> {
        let now = to_i64(now_unix_ms, "created_at_unix_ms")?;
        self.persistence.with_write_transaction(|transaction| {
            let active: i64 = transaction
                .query_row(
                    "SELECT COUNT(*) FROM messages
                     WHERE conversation_id = ?1 AND status = 'generating'",
                    params![conversation_id.to_string()],
                    |row| row.get(0),
                )
                .map_err(|source| PersistenceError::sqlite("check_active_generation", source))?;
            if active > 0 {
                return Err(PersistenceError::RecordConflict {
                    entity: "chat_generation",
                });
            }
            let sequence = allocate_sequence(transaction, conversation_id, now)?;
            transaction
                .execute(
                    "INSERT INTO messages
                     (message_id, conversation_id, role, status, sequence, content,
                      generation_id, cancellation_requested, last_event_sequence, failure_code,
                      created_at_unix_ms, updated_at_unix_ms, completed_at_unix_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, '', ?6, 0, 0, NULL, ?7, ?7, NULL)",
                    params![
                        message_id.to_string(),
                        conversation_id.to_string(),
                        PersistedChatRole::Assistant.as_str(),
                        PersistedChatMessageStatus::Generating.as_str(),
                        sequence,
                        generation_id.to_string(),
                        now,
                    ],
                )
                .map_err(|source| PersistenceError::sqlite("insert_assistant_message", source))?;
            read_message(transaction, message_id)
        })
    }

    /// Atomically appends a user message and opens its assistant generation.
    ///
    /// Conversation validation, active-generation exclusion, both sequence
    /// allocations, and both inserts share one transaction. A rejected
    /// generation therefore cannot leave an orphaned user message.
    pub fn admit_generation(
        &self,
        conversation_id: ConversationId,
        user_message_id: MessageId,
        assistant_message_id: MessageId,
        generation_id: GenerationId,
        content: &str,
        now_unix_ms: u64,
    ) -> Result<PersistedGenerationAdmission, PersistenceError> {
        validate_content(content)?;
        if content.trim().is_empty() {
            return Err(PersistenceError::InvalidRecord { field: "content" });
        }
        let now = to_i64(now_unix_ms, "created_at_unix_ms")?;
        self.persistence.with_write_transaction(|transaction| {
            let active: i64 = transaction
                .query_row(
                    "SELECT COUNT(*) FROM messages
                     WHERE conversation_id = ?1 AND status = 'generating'",
                    params![conversation_id.to_string()],
                    |row| row.get(0),
                )
                .map_err(|source| PersistenceError::sqlite("check_active_generation", source))?;
            if active > 0 {
                return Err(PersistenceError::RecordConflict {
                    entity: "chat_generation",
                });
            }

            let user_sequence = allocate_sequence(transaction, conversation_id, now)?;
            transaction
                .execute(
                    "INSERT INTO messages
                     (message_id, conversation_id, role, status, sequence, content,
                      generation_id, cancellation_requested, last_event_sequence, failure_code,
                      created_at_unix_ms, updated_at_unix_ms, completed_at_unix_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, 0, 0, NULL, ?7, ?7, ?7)",
                    params![
                        user_message_id.to_string(),
                        conversation_id.to_string(),
                        PersistedChatRole::User.as_str(),
                        PersistedChatMessageStatus::Completed.as_str(),
                        user_sequence,
                        content,
                        now,
                    ],
                )
                .map_err(|source| PersistenceError::sqlite("admit_user_message", source))?;

            let assistant_sequence = allocate_sequence(transaction, conversation_id, now)?;
            transaction
                .execute(
                    "INSERT INTO messages
                     (message_id, conversation_id, role, status, sequence, content,
                      generation_id, cancellation_requested, last_event_sequence, failure_code,
                      created_at_unix_ms, updated_at_unix_ms, completed_at_unix_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, '', ?6, 0, 0, NULL, ?7, ?7, NULL)",
                    params![
                        assistant_message_id.to_string(),
                        conversation_id.to_string(),
                        PersistedChatRole::Assistant.as_str(),
                        PersistedChatMessageStatus::Generating.as_str(),
                        assistant_sequence,
                        generation_id.to_string(),
                        now,
                    ],
                )
                .map_err(|source| PersistenceError::sqlite("admit_assistant_message", source))?;

            Ok(PersistedGenerationAdmission {
                user_message: read_message(transaction, user_message_id)?,
                assistant_message: read_message(transaction, assistant_message_id)?,
            })
        })
    }

    /// Commits one bounded checkpoint of accumulated assistant content.
    ///
    /// Content is replaced wholesale inside a single transaction so a crash
    /// leaves a coherent partial rather than a torn write.
    pub fn checkpoint_assistant_content(
        &self,
        generation_id: GenerationId,
        content: &str,
        last_event_sequence: u64,
        now_unix_ms: u64,
    ) -> Result<(), PersistenceError> {
        validate_content(content)?;
        let now = to_i64(now_unix_ms, "updated_at_unix_ms")?;
        let sequence = to_i64(last_event_sequence, "last_event_sequence")?;
        self.persistence.with_write_transaction(|transaction| {
            let changed = transaction
                .execute(
                    "UPDATE messages
                     SET content = ?2,
                         last_event_sequence = MAX(?3, last_event_sequence),
                         updated_at_unix_ms = MAX(?4, updated_at_unix_ms)
                     WHERE generation_id = ?1 AND status = 'generating'",
                    params![generation_id.to_string(), content, sequence, now],
                )
                .map_err(|source| PersistenceError::sqlite("checkpoint_assistant", source))?;
            if changed == 0 {
                return Err(PersistenceError::RecordConflict {
                    entity: "chat_generation",
                });
            }
            touch_conversation(transaction, generation_id, now)
        })
    }

    /// Commits one terminal outcome for an assistant generation.
    pub fn finish_assistant_generation(
        &self,
        generation_id: GenerationId,
        outcome: PersistedGenerationOutcome,
        content: &str,
        failure_code: Option<&str>,
        last_event_sequence: u64,
        now_unix_ms: u64,
    ) -> Result<PersistedChatMessage, PersistenceError> {
        validate_content(content)?;
        if let Some(code) = failure_code {
            validate_text("failure_code", code, FAILURE_CODE_BYTES_MAX)?;
        }
        if failure_code.is_some() && outcome != PersistedGenerationOutcome::Failed {
            return Err(PersistenceError::InvalidRecord {
                field: "failure_code",
            });
        }
        let now = to_i64(now_unix_ms, "completed_at_unix_ms")?;
        let sequence = to_i64(last_event_sequence, "last_event_sequence")?;
        self.persistence.with_write_transaction(|transaction| {
            let changed = transaction
                .execute(
                    "UPDATE messages
                     SET status = ?2, content = ?3, failure_code = ?4,
                         last_event_sequence = MAX(?5, last_event_sequence),
                         updated_at_unix_ms = MAX(?6, updated_at_unix_ms),
                         completed_at_unix_ms = ?6
                     WHERE generation_id = ?1 AND status = 'generating'",
                    params![
                        generation_id.to_string(),
                        outcome.status().as_str(),
                        content,
                        failure_code,
                        sequence,
                        now,
                    ],
                )
                .map_err(|source| PersistenceError::sqlite("finish_assistant", source))?;
            if changed == 0 {
                return Err(PersistenceError::RecordConflict {
                    entity: "chat_generation",
                });
            }
            touch_conversation(transaction, generation_id, now)?;
            read_message_by_generation(transaction, generation_id)
        })
    }

    /// Records cooperative cancellation intent for one active generation.
    pub fn request_cancellation(
        &self,
        generation_id: GenerationId,
        now_unix_ms: u64,
    ) -> Result<bool, PersistenceError> {
        let now = to_i64(now_unix_ms, "updated_at_unix_ms")?;
        self.persistence.with_write_transaction(|transaction| {
            let changed = transaction
                .execute(
                    "UPDATE messages
                     SET cancellation_requested = 1,
                         updated_at_unix_ms = MAX(?2, updated_at_unix_ms)
                     WHERE generation_id = ?1 AND status = 'generating'",
                    params![generation_id.to_string(), now],
                )
                .map_err(|source| PersistenceError::sqlite("request_cancellation", source))?;
            Ok(changed > 0)
        })
    }

    /// Returns the assistant message owning one generation attempt.
    pub fn message_by_generation(
        &self,
        generation_id: GenerationId,
    ) -> Result<Option<PersistedChatMessage>, PersistenceError> {
        self.persistence.with_connection(|connection| {
            connection
                .query_row(
                    &format!("SELECT {MESSAGE_COLUMNS} FROM messages WHERE generation_id = ?1"),
                    params![generation_id.to_string()],
                    decode_message,
                )
                .optional()
                .map_err(|source| PersistenceError::sqlite("read_message_by_generation", source))?
                .transpose()
        })
    }

    /// Reclassifies every generation interrupted by an earlier process exit.
    ///
    /// A message left in the generating state can never recover to completed,
    /// because no terminal provider evidence was ever observed for it.
    pub fn recover_interrupted_generations(
        &self,
        now_unix_ms: u64,
    ) -> Result<u32, PersistenceError> {
        let now = to_i64(now_unix_ms, "updated_at_unix_ms")?;
        self.persistence.with_write_transaction(|transaction| {
            let changed = transaction
                .execute(
                    "UPDATE messages
                     SET status = CASE
                             WHEN cancellation_requested = 1 THEN 'cancelled'
                             ELSE 'failed'
                         END,
                         failure_code = CASE
                             WHEN cancellation_requested = 1 THEN NULL
                             ELSE ?2
                         END,
                         updated_at_unix_ms = MAX(?1, updated_at_unix_ms),
                         completed_at_unix_ms = ?1
                     WHERE status = 'generating'",
                    params![now, INTERRUPTED_FAILURE_CODE],
                )
                .map_err(|source| PersistenceError::sqlite("recover_interrupted", source))?;
            u32::try_from(changed).map_err(|_| PersistenceError::InvalidRecord {
                field: "recovered_count",
            })
        })
    }
}

fn allocate_sequence(
    transaction: &rusqlite::Transaction<'_>,
    conversation_id: ConversationId,
    now: i64,
) -> Result<i64, PersistenceError> {
    let sequence: i64 = transaction
        .query_row(
            "SELECT next_sequence FROM conversations WHERE conversation_id = ?1",
            params![conversation_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|source| PersistenceError::sqlite("read_next_sequence", source))?
        .ok_or(PersistenceError::RecordNotFound {
            entity: "conversation",
        })?;
    transaction
        .execute(
            "UPDATE conversations
             SET next_sequence = next_sequence + 1,
                 updated_at_unix_ms = MAX(?2, updated_at_unix_ms)
             WHERE conversation_id = ?1",
            params![conversation_id.to_string(), now],
        )
        .map_err(|source| PersistenceError::sqlite("advance_next_sequence", source))?;
    Ok(sequence)
}

fn touch_conversation(
    transaction: &rusqlite::Transaction<'_>,
    generation_id: GenerationId,
    now: i64,
) -> Result<(), PersistenceError> {
    transaction
        .execute(
            "UPDATE conversations
             SET updated_at_unix_ms = MAX(?2, updated_at_unix_ms)
             WHERE conversation_id = (
                 SELECT conversation_id FROM messages WHERE generation_id = ?1
             )",
            params![generation_id.to_string(), now],
        )
        .map_err(|source| PersistenceError::sqlite("touch_conversation", source))?;
    Ok(())
}

fn read_message(
    transaction: &rusqlite::Transaction<'_>,
    message_id: MessageId,
) -> Result<PersistedChatMessage, PersistenceError> {
    transaction
        .query_row(
            &format!("SELECT {MESSAGE_COLUMNS} FROM messages WHERE message_id = ?1"),
            params![message_id.to_string()],
            decode_message,
        )
        .map_err(|source| PersistenceError::sqlite("read_message", source))?
}

fn read_message_by_generation(
    transaction: &rusqlite::Transaction<'_>,
    generation_id: GenerationId,
) -> Result<PersistedChatMessage, PersistenceError> {
    transaction
        .query_row(
            &format!("SELECT {MESSAGE_COLUMNS} FROM messages WHERE generation_id = ?1"),
            params![generation_id.to_string()],
            decode_message,
        )
        .map_err(|source| PersistenceError::sqlite("read_message_by_generation", source))?
}

type DecodedConversation = Result<PersistedConversation, PersistenceError>;

fn decode_conversation(row: &Row<'_>) -> rusqlite::Result<DecodedConversation> {
    let conversation_id: String = row.get(0)?;
    let title: String = row.get(1)?;
    let canonical_model_id: Option<String> = row.get(2)?;
    let runtime_provider_id: String = row.get(3)?;
    let created: i64 = row.get(4)?;
    let updated: i64 = row.get(5)?;
    let message_count: i64 = row.get(6)?;
    let model_display_name: Option<String> = row.get(7)?;
    let model_family: Option<String> = row.get(8)?;

    Ok((|| {
        Ok(PersistedConversation {
            conversation_id: parse_conversation_id(&conversation_id)?,
            title,
            canonical_model_id,
            model_display_name,
            model_family,
            runtime_provider_id: RuntimeProviderId::new(runtime_provider_id),
            message_count: u32::try_from(message_count).map_err(|_| {
                PersistenceError::InvalidRecord {
                    field: "message_count",
                }
            })?,
            created_at_unix_ms: to_u64(created, "created_at_unix_ms")?,
            updated_at_unix_ms: to_u64(updated, "updated_at_unix_ms")?,
        })
    })())
}

type DecodedMessage = Result<PersistedChatMessage, PersistenceError>;

fn decode_message(row: &Row<'_>) -> rusqlite::Result<DecodedMessage> {
    let message_id: String = row.get(0)?;
    let conversation_id: String = row.get(1)?;
    let role: String = row.get(2)?;
    let status: String = row.get(3)?;
    let sequence: i64 = row.get(4)?;
    let content: String = row.get(5)?;
    let generation_id: Option<String> = row.get(6)?;
    let cancellation_requested: bool = row.get(7)?;
    let last_event_sequence: i64 = row.get(8)?;
    let failure_code: Option<String> = row.get(9)?;
    let created: i64 = row.get(10)?;
    let updated: i64 = row.get(11)?;
    let completed: Option<i64> = row.get(12)?;

    Ok((|| {
        Ok(PersistedChatMessage {
            message_id: parse_message_id(&message_id)?,
            conversation_id: parse_conversation_id(&conversation_id)?,
            role: PersistedChatRole::parse(&role)?,
            status: PersistedChatMessageStatus::parse(&status)?,
            sequence: u32::try_from(sequence)
                .map_err(|_| PersistenceError::InvalidRecord { field: "sequence" })?,
            content,
            generation_id: generation_id
                .as_deref()
                .map(parse_generation_id)
                .transpose()?,
            cancellation_requested,
            last_event_sequence: to_u64(last_event_sequence, "last_event_sequence")?,
            failure_code,
            created_at_unix_ms: to_u64(created, "created_at_unix_ms")?,
            updated_at_unix_ms: to_u64(updated, "updated_at_unix_ms")?,
            completed_at_unix_ms: completed
                .map(|value| to_u64(value, "completed_at_unix_ms"))
                .transpose()?,
        })
    })())
}

fn parse_conversation_id(value: &str) -> Result<ConversationId, PersistenceError> {
    value.parse().map_err(|_| PersistenceError::InvalidRecord {
        field: "conversation_id",
    })
}

fn parse_message_id(value: &str) -> Result<MessageId, PersistenceError> {
    value.parse().map_err(|_| PersistenceError::InvalidRecord {
        field: "message_id",
    })
}

fn parse_generation_id(value: &str) -> Result<GenerationId, PersistenceError> {
    value.parse().map_err(|_| PersistenceError::InvalidRecord {
        field: "generation_id",
    })
}

fn validate_title(value: &str) -> Result<(), PersistenceError> {
    validate_text("title", value, TITLE_BYTES_MAX)
}

fn validate_text(
    field: &'static str,
    value: &str,
    maximum_bytes: usize,
) -> Result<(), PersistenceError> {
    if value.is_empty() || value.len() > maximum_bytes || value.chars().any(char::is_control) {
        Err(PersistenceError::InvalidRecord { field })
    } else {
        Ok(())
    }
}

/// Validates bounded message text while allowing ordinary line structure.
fn validate_content(value: &str) -> Result<(), PersistenceError> {
    if value.len() > CONTENT_BYTES_MAX {
        return Err(PersistenceError::InvalidRecord { field: "content" });
    }
    if value
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(PersistenceError::InvalidRecord { field: "content" });
    }
    Ok(())
}

fn to_i64(value: u64, field: &'static str) -> Result<i64, PersistenceError> {
    i64::try_from(value).map_err(|_| PersistenceError::InvalidRecord { field })
}

fn to_u64(value: i64, field: &'static str) -> Result<u64, PersistenceError> {
    u64::try_from(value).map_err(|_| PersistenceError::InvalidRecord { field })
}
