//! Deterministic bounded context assembly.
//!
//! Context is built only from persisted messages of the selected conversation.
//! No file is read, no retrieval is performed, and no model summarizes history.

use gixgiz_contracts::{ChatWarning, ChatWarningCode};
use gixgiz_persistence::{PersistedChatMessage, PersistedChatMessageStatus, PersistedChatRole};
use gixgiz_runtime::{RuntimeChatMessage, RuntimeChatRole};

/// Largest number of prior messages admitted into one generation context.
pub(super) const MAX_CONTEXT_MESSAGES: usize = 20;

/// Largest total context payload admitted into one generation.
pub(super) const MAX_CONTEXT_BYTES: usize = 48 * 1024;

/// Bounded context plus any warning raised while assembling it.
pub(super) struct BoundedContext {
    pub(super) messages: Vec<RuntimeChatMessage>,
    pub(super) warnings: Vec<ChatWarning>,
}

/// Builds bounded context from ordered persisted history.
///
/// Only completed messages are admitted: a cancelled or failed assistant
/// message holds partial text that would misrepresent the conversation. The
/// newest complete messages that fit are retained, and chronological order is
/// restored before the provider call.
#[cfg(test)]
pub(super) fn build(history: &[PersistedChatMessage]) -> BoundedContext {
    build_inner(history, None)
}

/// Builds bounded context while reserving space for the user message whose
/// durable admission has not occurred yet.
pub(super) fn build_with_user(
    history: &[PersistedChatMessage],
    user_content: &str,
) -> Result<BoundedContext, ()> {
    if user_content.len() > MAX_CONTEXT_BYTES {
        return Err(());
    }
    Ok(build_inner(history, Some(user_content)))
}

fn build_inner(history: &[PersistedChatMessage], user_content: Option<&str>) -> BoundedContext {
    let mut selected = Vec::new();
    let mut total_bytes = user_content.map_or(0, str::len);
    let prior_limit = MAX_CONTEXT_MESSAGES - usize::from(user_content.is_some());
    let mut dropped = false;

    for message in history.iter().rev() {
        if message.status != PersistedChatMessageStatus::Completed {
            continue;
        }
        let Some(role) = runtime_role(message.role) else {
            continue;
        };
        if selected.len() >= prior_limit {
            dropped = true;
            break;
        }
        let Some(next_total) = total_bytes.checked_add(message.content.len()) else {
            dropped = true;
            break;
        };
        if next_total > MAX_CONTEXT_BYTES {
            dropped = true;
            break;
        }
        total_bytes = next_total;
        selected.push(RuntimeChatMessage {
            role,
            content: message.content.clone(),
        });
    }

    selected.reverse();
    if let Some(content) = user_content {
        selected.push(RuntimeChatMessage {
            role: RuntimeChatRole::User,
            content: content.to_owned(),
        });
    }
    let mut warnings = Vec::new();
    if dropped {
        warnings.push(ChatWarning {
            code: ChatWarningCode::ContextTruncated,
            message: "Older messages were left out to keep this reply within safe limits."
                .to_owned(),
        });
    }
    if history.iter().any(|message| {
        message.status.is_terminal() && message.status != PersistedChatMessageStatus::Completed
    }) {
        warnings.push(ChatWarning {
            code: ChatWarningCode::PartialAssistantContent,
            message:
                "This conversation contains an incomplete reply, which is not reused as context."
                    .to_owned(),
        });
    }

    BoundedContext {
        messages: selected,
        warnings,
    }
}

const fn runtime_role(role: PersistedChatRole) -> Option<RuntimeChatRole> {
    match role {
        PersistedChatRole::User => Some(RuntimeChatRole::User),
        PersistedChatRole::Assistant => Some(RuntimeChatRole::Assistant),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use gixgiz_contracts::{ConversationId, GenerationId, MessageId};

    use super::*;

    fn message(
        sequence: u32,
        role: PersistedChatRole,
        status: PersistedChatMessageStatus,
        content: &str,
    ) -> PersistedChatMessage {
        PersistedChatMessage {
            message_id: MessageId::new(),
            conversation_id: ConversationId::new(),
            role,
            status,
            sequence,
            content: content.to_owned(),
            generation_id: matches!(role, PersistedChatRole::Assistant).then(GenerationId::new),
            cancellation_requested: false,
            last_event_sequence: 0,
            failure_code: None,
            created_at_unix_ms: 1,
            updated_at_unix_ms: 1,
            completed_at_unix_ms: status.is_terminal().then_some(1),
        }
    }

    #[test]
    fn completed_history_is_preserved_in_chronological_order() {
        let history = vec![
            message(
                1,
                PersistedChatRole::User,
                PersistedChatMessageStatus::Completed,
                "first",
            ),
            message(
                2,
                PersistedChatRole::Assistant,
                PersistedChatMessageStatus::Completed,
                "second",
            ),
            message(
                3,
                PersistedChatRole::User,
                PersistedChatMessageStatus::Completed,
                "third",
            ),
        ];

        let context = build(&history);
        let contents: Vec<&str> = context
            .messages
            .iter()
            .map(|message| message.content.as_str())
            .collect();

        assert_eq!(contents, vec!["first", "second", "third"]);
        assert!(context.warnings.is_empty());
    }

    #[test]
    fn partial_assistant_output_is_never_reused_as_context() {
        let history = vec![
            message(
                1,
                PersistedChatRole::User,
                PersistedChatMessageStatus::Completed,
                "keep",
            ),
            message(
                2,
                PersistedChatRole::Assistant,
                PersistedChatMessageStatus::Cancelled,
                "half writ",
            ),
            message(
                3,
                PersistedChatRole::Assistant,
                PersistedChatMessageStatus::Failed,
                "broken",
            ),
            message(
                4,
                PersistedChatRole::Assistant,
                PersistedChatMessageStatus::Generating,
                "streaming",
            ),
        ];

        let context = build(&history);

        assert_eq!(context.messages.len(), 1);
        assert_eq!(context.messages[0].content, "keep");
        assert!(
            context
                .warnings
                .iter()
                .any(|warning| warning.code == ChatWarningCode::PartialAssistantContent)
        );
    }

    #[test]
    fn oldest_messages_are_dropped_when_the_message_bound_is_reached() {
        let history: Vec<PersistedChatMessage> = (1..=25)
            .map(|index| {
                message(
                    index,
                    PersistedChatRole::User,
                    PersistedChatMessageStatus::Completed,
                    &format!("message {index}"),
                )
            })
            .collect();

        let context = build(&history);

        assert_eq!(context.messages.len(), MAX_CONTEXT_MESSAGES);
        assert_eq!(context.messages[0].content, "message 6");
        assert_eq!(
            context.messages[MAX_CONTEXT_MESSAGES - 1].content,
            "message 25"
        );
        assert!(
            context
                .warnings
                .iter()
                .any(|warning| warning.code == ChatWarningCode::ContextTruncated)
        );
    }

    #[test]
    fn oversized_history_is_bounded_by_total_bytes() {
        let large = "x".repeat(20 * 1024);
        let history: Vec<PersistedChatMessage> = (1..=5)
            .map(|index| {
                message(
                    index,
                    PersistedChatRole::User,
                    PersistedChatMessageStatus::Completed,
                    &large,
                )
            })
            .collect();

        let context = build(&history);
        let total: usize = context
            .messages
            .iter()
            .map(|message| message.content.len())
            .sum();

        assert!(total <= MAX_CONTEXT_BYTES);
        assert_eq!(context.messages.len(), 2);
        assert!(
            context
                .warnings
                .iter()
                .any(|warning| warning.code == ChatWarningCode::ContextTruncated)
        );
    }

    #[test]
    fn empty_history_produces_empty_context_without_warnings() {
        let context = build(&[]);

        assert!(context.messages.is_empty());
        assert!(context.warnings.is_empty());
    }
}
