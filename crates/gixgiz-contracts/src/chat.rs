use std::{fmt, str::FromStr};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    CandidateModelId, CorrelationId, RequestId, RuntimeDisplayName, RuntimeProviderId,
    SafeErrorPayload,
};

/// Schema generation for the local conversation and streaming-generation contracts.
pub const CHAT_SCHEMA_VERSION: u32 = 1;

/// Largest accepted user message in bytes.
pub const MAX_USER_MESSAGE_BYTES: usize = 16 * 1024;

/// Largest retained assistant response in bytes.
pub const MAX_ASSISTANT_OUTPUT_BYTES: usize = 64 * 1024;

/// Largest accepted conversation title in bytes.
pub const MAX_CONVERSATION_TITLE_BYTES: usize = 120;

/// Largest conversation page a single list request may return.
pub const MAX_CONVERSATIONS_PER_PAGE: u16 = 50;

/// Largest bounded generation-event page a single observation request may return.
pub const MAX_GENERATION_EVENT_PAGE: u16 = 64;

/// Opaque identifier for one local conversation.
#[derive(Clone, Copy, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConversationId(Uuid);

impl ConversationId {
    /// Generates a random opaque version-four identifier.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates an identifier from a UUID for deterministic callers.
    #[must_use]
    pub const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    /// Returns the underlying UUID value.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for ConversationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ConversationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for ConversationId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}

/// Opaque identifier for one persisted conversation message.
#[derive(Clone, Copy, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MessageId(Uuid);

impl MessageId {
    /// Generates a random opaque version-four identifier.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates an identifier from a UUID for deterministic callers.
    #[must_use]
    pub const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    /// Returns the underlying UUID value.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for MessageId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}

/// Opaque identifier for one bounded assistant generation attempt.
#[derive(Clone, Copy, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GenerationId(Uuid);

impl GenerationId {
    /// Generates a random opaque version-four identifier.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates an identifier from a UUID for deterministic callers.
    #[must_use]
    pub const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    /// Returns the underlying UUID value.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for GenerationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for GenerationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for GenerationId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}

/// Author of one conversation message.
///
/// The v0.1 conversation model contains no system or tool authorship.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ChatRole {
    /// Content written by the person using GixGiz.
    User,
    /// Content produced by the selected local model.
    Assistant,
    /// A newer peer supplied an unrecognized role.
    #[serde(other)]
    Unknown,
}

/// Durable lifecycle state of one conversation message.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ChatMessageStatus {
    /// The message is persisted and no generation has started.
    Pending,
    /// A bounded assistant generation is currently producing content.
    Generating,
    /// The message reached a complete, verified terminal state.
    Completed,
    /// Explicit cancellation stopped generation; retained content is partial.
    Cancelled,
    /// Generation failed safely; retained content is partial or empty.
    Failed,
    /// A newer peer supplied an unrecognized status.
    #[serde(other)]
    Unknown,
}

impl ChatMessageStatus {
    /// Reports whether no further content can be appended to this message.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed)
    }

    /// Reports whether the retained content is known to be incomplete.
    #[must_use]
    pub const fn is_partial(self) -> bool {
        matches!(self, Self::Cancelled | Self::Failed)
    }
}

/// Ordered event emitted while one assistant generation runs.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ChatGenerationEventKind {
    /// The generation was admitted and the provider request began.
    Started,
    /// A bounded increment of assistant text is available.
    Delta,
    /// The provider produced a valid terminal completion.
    Completed,
    /// Cooperative cancellation stopped the generation.
    Cancelled,
    /// The generation failed safely.
    Failed,
    /// An inactivity or total-duration bound was exceeded.
    TimedOut,
    /// The provider stopped, but the authoritative terminal database write failed.
    ///
    /// This closes the transient stream without claiming a durable terminal
    /// state. Startup recovery classifies the still-generating SQLite record.
    DurabilityInterrupted,
    /// A newer peer supplied an unrecognized event kind.
    #[serde(other)]
    Unknown,
}

/// Terminal outcome of one bounded generation attempt.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ChatGenerationTerminalState {
    /// The provider produced a valid terminal completion.
    Completed,
    /// Cooperative cancellation stopped the generation.
    Cancelled,
    /// The generation failed safely.
    Failed,
    /// An inactivity or total-duration bound was exceeded.
    TimedOut,
    /// A newer peer supplied an unrecognized terminal state.
    #[serde(other)]
    Unknown,
}

/// Where the active conversation is executed.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ChatLocalityStatus {
    /// Generation runs against a verified loopback runtime on this device.
    ///
    /// This asserts local execution only. It does not assert that the device is
    /// disconnected from every network.
    RunningLocally,
    /// Locality could not be established from current evidence.
    #[serde(other)]
    Unknown,
}

/// Stable machine-readable reason a chat operation could not proceed.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ChatFailureCode {
    /// The local runtime is not reachable or not ready.
    RuntimeUnavailable,
    /// The verified runtime version or capability set no longer satisfies policy.
    RuntimeIncompatible,
    /// Recorded reuse consent does not currently permit provider access.
    RuntimeConsentRequired,
    /// The conversation's model is no longer available from the provider.
    ModelUnavailable,
    /// The provider mapping for the conversation's model changed since setup.
    ModelChanged,
    /// The generation exceeded an inactivity or total-duration bound.
    GenerationTimedOut,
    /// The provider stream ended before a valid terminal completion.
    ProviderDisconnected,
    /// The provider produced an event that failed bounded validation.
    MalformedProviderStream,
    /// The submitted user message exceeded the accepted size bound.
    MessageTooLarge,
    /// The assembled context exceeded the accepted size bound.
    ContextTooLarge,
    /// The requested conversation does not exist.
    ConversationNotFound,
    /// The requested generation does not exist.
    GenerationNotFound,
    /// Another generation is already active for this conversation.
    GenerationAlreadyActive,
    /// Durable conversation storage is unavailable.
    PersistenceUnavailable,
    /// The operation stopped because cancellation was requested.
    Cancelled,
    /// A newer peer supplied an unrecognized failure code.
    #[serde(other)]
    Unknown,
}

/// Recommended next action after a chat failure.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ChatRecoveryAction {
    /// Send the message again without other changes.
    RetryGeneration,
    /// Review local runtime status before retrying.
    CheckRuntime,
    /// Run the model setup workflow again before chatting.
    RunModelSetup,
    /// Shorten the message or start a new conversation.
    ShortenMessage,
    /// Start a new conversation to continue.
    StartNewConversation,
    /// Restart GixGiz to recover a healthy session.
    RestartApplication,
    /// No user action changes the outcome.
    NoAction,
    /// A newer peer supplied an unrecognized recovery action.
    #[serde(other)]
    Unknown,
}

/// Stable machine-readable chat warning.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ChatWarningCode {
    /// Older messages were excluded because the context bound was reached.
    ContextTruncated,
    /// Retained assistant content is incomplete.
    PartialAssistantContent,
    /// A previously running generation was interrupted before this session started.
    InterruptedGenerationRecovered,
    /// A newer peer supplied an unrecognized warning code.
    #[serde(other)]
    Unknown,
}

/// Safe non-blocking chat warning shown alongside conversation state.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ChatWarning {
    /// Stable machine-readable warning code.
    pub code: ChatWarningCode,
    /// Bounded plain-language explanation safe for display.
    pub message: String,
}

/// Bounded identity of the model bound to one conversation.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ChatModelIdentity {
    /// Canonical catalogue identity verified by the setup workflow.
    pub canonical_model_id: CandidateModelId,
    /// Bounded display name safe for beginner-facing presentation.
    pub display_name: String,
    /// Bounded model family label.
    pub family: String,
}

/// Bounded runtime and locality status presented beside a conversation.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ChatRuntimeStatus {
    /// Chat schema generation.
    pub schema_version: u32,
    /// Stable provider identity serving this conversation.
    pub provider_id: RuntimeProviderId,
    /// Bounded provider display name.
    pub runtime_display_name: RuntimeDisplayName,
    /// Verified model identity when one is bound and available.
    pub model: Option<ChatModelIdentity>,
    /// Where generation executes.
    pub locality: ChatLocalityStatus,
    /// Whether a new generation may currently start.
    pub ready: bool,
    /// Stable failure code explaining why generation is unavailable.
    pub blocked_by: Option<ChatFailureCode>,
    /// Recommended action when generation is unavailable.
    pub recovery_action: Option<ChatRecoveryAction>,
}

/// Authenticated request for provider-neutral chat locality/readiness.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ChatRuntimeStatusRequest {
    /// Conversation whose bound model should be checked, when selected.
    pub conversation_id: Option<ConversationId>,
    /// Identifier shared with the response and safe failures.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Authoritative chat locality/readiness with boundary identifiers.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ChatRuntimeStatusResponse {
    /// Provider-neutral status derived by Rust.
    pub status: ChatRuntimeStatus,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// One persisted conversation message.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Durable message identifier.
    pub message_id: MessageId,
    /// Owning conversation.
    pub conversation_id: ConversationId,
    /// Message author.
    pub role: ChatRole,
    /// Durable lifecycle state.
    pub status: ChatMessageStatus,
    /// Deterministic order within the conversation, starting at one.
    pub sequence: u32,
    /// Bounded message text.
    ///
    /// For an assistant message that is generating, cancelled, or failed this
    /// holds the accumulated partial output and must be presented as incomplete.
    pub content: String,
    /// Generation attempt that produced an assistant message.
    pub generation_id: Option<GenerationId>,
    /// Highest generation event sequence committed with this message.
    ///
    /// A client may use this cursor for bounded replay after reloading an
    /// active generation from authoritative SQLite state.
    pub last_event_sequence: u64,
    /// UTC Unix timestamp in milliseconds when the message was created.
    pub created_at_unix_ms: u64,
    /// UTC Unix timestamp in milliseconds when the message last changed.
    pub updated_at_unix_ms: u64,
    /// UTC Unix timestamp in milliseconds when a terminal state was committed.
    pub completed_at_unix_ms: Option<u64>,
}

/// Bounded conversation entry used by the conversation list.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ConversationSummary {
    /// Durable conversation identifier.
    pub conversation_id: ConversationId,
    /// Bounded conversation title.
    pub title: String,
    /// Number of persisted messages in the conversation.
    pub message_count: u32,
    /// Verified model identity bound to the conversation.
    pub model: Option<ChatModelIdentity>,
    /// UTC Unix timestamp in milliseconds when the conversation was created.
    pub created_at_unix_ms: u64,
    /// UTC Unix timestamp in milliseconds when the conversation last changed.
    pub updated_at_unix_ms: u64,
}

/// Authoritative persisted conversation state.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ConversationSnapshot {
    /// Chat schema generation.
    pub schema_version: u32,
    /// Durable conversation identifier.
    pub conversation_id: ConversationId,
    /// Bounded conversation title.
    pub title: String,
    /// Verified model identity bound to the conversation.
    pub model: Option<ChatModelIdentity>,
    /// Ordered persisted messages.
    pub messages: Vec<ChatMessage>,
    /// Generation currently producing content, when one is active.
    pub active_generation_id: Option<GenerationId>,
    /// Safe warnings describing truncation, partial content, or recovery.
    pub warnings: Vec<ChatWarning>,
    /// UTC Unix timestamp in milliseconds when the conversation was created.
    pub created_at_unix_ms: u64,
    /// UTC Unix timestamp in milliseconds when the conversation last changed.
    pub updated_at_unix_ms: u64,
}

/// Ordered transport event describing one bounded generation attempt.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ChatGenerationEvent {
    /// Chat schema generation.
    pub schema_version: u32,
    /// Generation attempt this event belongs to.
    pub generation_id: GenerationId,
    /// Conversation owning the generation.
    pub conversation_id: ConversationId,
    /// Assistant message receiving the generated content.
    pub assistant_message_id: MessageId,
    /// Identifier shared by the whole generation and its safe failures.
    pub correlation_id: CorrelationId,
    /// Strictly increasing sequence starting at one.
    pub sequence: u64,
    /// Event classification.
    pub kind: ChatGenerationEventKind,
    /// Bounded assistant text increment carried by a delta event.
    pub delta: Option<String>,
    /// Terminal outcome present only on a terminal event.
    pub terminal_state: Option<ChatGenerationTerminalState>,
    /// Safe boundary error present only on a failing terminal event.
    pub error: Option<SafeErrorPayload>,
    /// UTC Unix timestamp in milliseconds assigned by Rust.
    pub occurred_at_unix_ms: u64,
}

/// Authenticated request creating one empty local conversation.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct CreateConversationRequest {
    /// Optional bounded initial title; Rust assigns a safe default when absent.
    pub title: Option<String>,
    /// Identifier shared with the response and safe failures.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Authoritative snapshot of the created conversation.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct CreateConversationResponse {
    /// Authoritative persisted conversation.
    pub conversation: ConversationSnapshot,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Authenticated request for a bounded conversation page.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ListConversationsRequest {
    /// Maximum conversations to return, bounded by [`MAX_CONVERSATIONS_PER_PAGE`].
    pub limit: u16,
    /// Identifier shared with the response and safe failures.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Bounded conversation page ordered by most recent activity.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ListConversationsResponse {
    /// Chat schema generation.
    pub schema_version: u32,
    /// Bounded conversation entries, most recently updated first.
    pub conversations: Vec<ConversationSummary>,
    /// Whether more conversations exist beyond the returned page.
    pub truncated: bool,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Authenticated request for one authoritative conversation snapshot.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct GetConversationRequest {
    /// Conversation being read.
    pub conversation_id: ConversationId,
    /// Identifier shared with the response and safe failures.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Authoritative conversation snapshot.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct GetConversationResponse {
    /// Authoritative persisted conversation.
    pub conversation: ConversationSnapshot,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Authenticated request renaming one conversation.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RenameConversationRequest {
    /// Conversation being renamed.
    pub conversation_id: ConversationId,
    /// Bounded replacement title.
    pub title: String,
    /// Identifier shared with the response and safe failures.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Authoritative conversation snapshot after renaming.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RenameConversationResponse {
    /// Authoritative persisted conversation.
    pub conversation: ConversationSnapshot,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Authenticated request deleting one conversation and its owned messages.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct DeleteConversationRequest {
    /// Conversation being deleted.
    pub conversation_id: ConversationId,
    /// Identifier shared with the response and safe failures.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Bounded description of what an explicit conversation deletion removed.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct DeleteConversationResponse {
    /// Conversation that was removed.
    pub conversation_id: ConversationId,
    /// Number of owned messages removed with the conversation.
    pub deleted_message_count: u32,
    /// Whether the deletion removed durable records.
    pub deleted: bool,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Authenticated request appending one user message and starting generation.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SendMessageRequest {
    /// Conversation receiving the message.
    pub conversation_id: ConversationId,
    /// Bounded user message text.
    pub content: String,
    /// Identifier shared by the generation and its events.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Persisted message pair and the admitted generation attempt.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct SendMessageResponse {
    /// Chat schema generation.
    pub schema_version: u32,
    /// Persisted user message.
    pub user_message: ChatMessage,
    /// Assistant message admitted in the generating state.
    pub assistant_message: ChatMessage,
    /// Admitted generation attempt to observe.
    pub generation_id: GenerationId,
    /// Safe warnings raised while assembling bounded context.
    pub warnings: Vec<ChatWarning>,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Authenticated request for a bounded ordered generation-event page.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ChatGenerationEventsRequest {
    /// Generation being observed.
    pub generation_id: GenerationId,
    /// Exclusive lower bound; the response starts strictly after this sequence.
    pub after_sequence: u64,
    /// Maximum events to return, bounded by [`MAX_GENERATION_EVENT_PAGE`].
    pub limit: u16,
    /// Identifier shared with the response and safe failures.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Bounded ordered generation-event page.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct ChatGenerationEventsResponse {
    /// Chat schema generation.
    pub schema_version: u32,
    /// Ordered events strictly after the requested cursor.
    pub events: Vec<ChatGenerationEvent>,
    /// Whether bounded retention discarded events before the returned page.
    ///
    /// Clients that observe this must reload the authoritative conversation
    /// snapshot rather than reconstructing state from events alone.
    pub replay_incomplete: bool,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

/// Authenticated request cancelling one active generation.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct CancelGenerationRequest {
    /// Generation being cancelled.
    pub generation_id: GenerationId,
    /// Identifier shared with the response and safe failures.
    pub correlation_id: CorrelationId,
    /// Identifier unique to this request.
    pub request_id: RequestId,
}

/// Outcome of one cooperative cancellation request.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct CancelGenerationResponse {
    /// Generation targeted by the request.
    pub generation_id: GenerationId,
    /// Whether cancellation intent was recorded for an active generation.
    pub accepted: bool,
    /// Identifier copied from the request.
    pub correlation_id: CorrelationId,
    /// Identifier copied from the request.
    pub request_id: RequestId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_enums_are_forward_compatible_without_becoming_completed() {
        let status: ChatMessageStatus =
            serde_json::from_str("\"future_status\"").expect("future status accepted");
        let kind: ChatGenerationEventKind =
            serde_json::from_str("\"future_kind\"").expect("future kind accepted");
        let terminal: ChatGenerationTerminalState =
            serde_json::from_str("\"future_terminal\"").expect("future terminal accepted");
        let role: ChatRole = serde_json::from_str("\"future_role\"").expect("future role accepted");
        let locality: ChatLocalityStatus =
            serde_json::from_str("\"future_locality\"").expect("future locality accepted");

        assert_eq!(status, ChatMessageStatus::Unknown);
        assert_eq!(kind, ChatGenerationEventKind::Unknown);
        assert_eq!(terminal, ChatGenerationTerminalState::Unknown);
        assert_eq!(role, ChatRole::Unknown);
        assert_eq!(locality, ChatLocalityStatus::Unknown);
        assert!(!status.is_terminal());
        assert!(!status.is_partial());
    }

    #[test]
    fn chat_identifiers_round_trip_as_opaque_uuids() {
        let conversation = ConversationId::from_uuid(Uuid::nil());
        let message = MessageId::from_uuid(Uuid::nil());
        let generation = GenerationId::from_uuid(Uuid::nil());

        for encoded in [
            serde_json::to_string(&conversation).expect("conversation id serializes"),
            serde_json::to_string(&message).expect("message id serializes"),
            serde_json::to_string(&generation).expect("generation id serializes"),
        ] {
            assert_eq!(encoded, "\"00000000-0000-0000-0000-000000000000\"");
        }

        assert_eq!(
            ConversationId::from_str("00000000-0000-0000-0000-000000000000")
                .expect("conversation id parses"),
            conversation
        );
    }

    #[test]
    fn terminal_and_partial_states_are_explicit() {
        assert!(ChatMessageStatus::Completed.is_terminal());
        assert!(ChatMessageStatus::Cancelled.is_terminal());
        assert!(ChatMessageStatus::Failed.is_terminal());
        assert!(!ChatMessageStatus::Generating.is_terminal());
        assert!(!ChatMessageStatus::Pending.is_terminal());

        assert!(ChatMessageStatus::Cancelled.is_partial());
        assert!(ChatMessageStatus::Failed.is_partial());
        assert!(!ChatMessageStatus::Completed.is_partial());
    }

    #[test]
    fn chat_schema_has_no_provider_payload_prompt_or_private_path() {
        for schema in [
            serde_json::to_string(&schemars::schema_for!(ConversationSnapshot))
                .expect("conversation schema serializes"),
            serde_json::to_string(&schemars::schema_for!(ChatGenerationEvent))
                .expect("generation event schema serializes"),
            serde_json::to_string(&schemars::schema_for!(ChatRuntimeStatus))
                .expect("runtime status schema serializes"),
        ] {
            let schema = schema.to_ascii_lowercase();
            for forbidden in [
                "ollama",
                "/api/",
                "ndjson",
                "raw_payload",
                "provider_logs",
                "provider_response",
                "prompt_text",
                "local_path",
                "bearer",
            ] {
                assert!(!schema.contains(forbidden), "schema contains {forbidden}");
            }
        }
    }
}
