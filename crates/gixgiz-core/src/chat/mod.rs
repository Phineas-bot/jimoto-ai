//! Rust-owned local chat sessions, generation orchestration, and cancellation.
//!
//! SQLite is authoritative for every conversation and message. The transport
//! event stream is a bounded convenience for live observation only.

mod context;
mod generation;

#[cfg(test)]
mod tests;

use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use gixgiz_contracts::{
    CHAT_SCHEMA_VERSION, CancelGenerationRequest, CancelGenerationResponse, CandidateModelId,
    ChatFailureCode, ChatGenerationEvent, ChatGenerationEventKind, ChatGenerationEventsRequest,
    ChatGenerationEventsResponse, ChatGenerationTerminalState, ChatLocalityStatus, ChatMessage,
    ChatMessageStatus, ChatModelIdentity, ChatRecoveryAction, ChatRole, ChatRuntimeStatus,
    ChatRuntimeStatusRequest, ChatRuntimeStatusResponse, ChatWarning, ChatWarningCode,
    ConversationId, ConversationSnapshot, ConversationSummary, CorrelationId,
    CreateConversationRequest, CreateConversationResponse, DeleteConversationRequest,
    DeleteConversationResponse, GenerationId, GetConversationRequest, GetConversationResponse,
    ListConversationsRequest, ListConversationsResponse, MAX_ASSISTANT_OUTPUT_BYTES,
    MAX_CONVERSATION_TITLE_BYTES, MAX_CONVERSATIONS_PER_PAGE, MAX_GENERATION_EVENT_PAGE,
    MAX_USER_MESSAGE_BYTES, MessageId, RenameConversationRequest, RenameConversationResponse,
    RequestId, RuntimeProviderId, SendMessageRequest, SendMessageResponse,
};
use gixgiz_persistence::{
    ChatRepository, PersistedChatMessage, PersistedChatMessageStatus, PersistedChatRole,
    PersistedConversationInput, PersistedGenerationOutcome, Persistence,
};
use gixgiz_runtime::{
    CHAT_DELTA_CHANNEL_CAPACITY, RuntimeCancellationToken, RuntimeChatRequest, RuntimeError,
    RuntimeOperationContext, RuntimeProvider,
};
use tokio::sync::mpsc;
use tracing::Instrument;

use crate::{CoreError, RuntimeService};
use context::{BoundedContext, MAX_CONTEXT_BYTES};
use generation::GenerationRegistry;

/// Total wall-clock bound for one generation attempt.
const GENERATION_TIMEOUT: Duration = Duration::from_secs(300);

/// Bound applied to short readiness and persistence calls.
const READINESS_TIMEOUT: Duration = Duration::from_secs(10);

/// Accumulated assistant bytes committed between durable checkpoints.
const CHECKPOINT_BYTES: usize = 2 * 1024;

/// Rust-owned local chat service.
#[derive(Clone)]
pub struct ChatService {
    runtime: RuntimeService,
    provider: Arc<dyn RuntimeProvider>,
    repository: ChatRepository,
    generations: GenerationRegistry,
}

impl ChatService {
    /// Composes chat with Rust-owned persistence and provider-neutral runtime policy.
    #[must_use]
    pub fn with_persistence(provider: Arc<dyn RuntimeProvider>, persistence: &Persistence) -> Self {
        Self {
            runtime: RuntimeService::with_persistence(provider.clone(), persistence),
            provider,
            repository: persistence.chat(),
            generations: GenerationRegistry::default(),
        }
    }

    /// Reclassifies generations interrupted by an earlier process exit.
    ///
    /// A message left generating can never become completed, because no
    /// terminal provider evidence was ever observed for it.
    pub async fn recover_interrupted(&self) -> Result<u32, CoreError> {
        let repository = self.repository.clone();
        let now = unix_millis();
        blocking(move || repository.recover_interrupted_generations(now))
            .await?
            .map_err(|_| CoreError::Chat(ChatFailureCode::PersistenceUnavailable))
    }

    /// Creates one empty conversation bound to the registered provider.
    pub async fn create_conversation(
        &self,
        request: CreateConversationRequest,
    ) -> Result<CreateConversationResponse, CoreError> {
        let title = normalize_title(request.title.as_deref().unwrap_or("New conversation"))?;
        let input = PersistedConversationInput {
            conversation_id: ConversationId::new(),
            title,
            canonical_model_id: None,
            runtime_provider_id: self.provider.provider_id().clone(),
            created_at_unix_ms: unix_millis(),
        };
        let repository = self.repository.clone();
        let conversation_id = input.conversation_id;
        blocking(move || repository.create_conversation(&input))
            .await?
            .map_err(persistence_failure)?;

        Ok(CreateConversationResponse {
            conversation: self.snapshot(conversation_id).await?,
            correlation_id: request.correlation_id,
            request_id: request.request_id,
        })
    }

    /// Returns a bounded conversation page ordered by most recent activity.
    pub async fn list_conversations(
        &self,
        request: ListConversationsRequest,
    ) -> Result<ListConversationsResponse, CoreError> {
        if request.limit == 0 || request.limit > MAX_CONVERSATIONS_PER_PAGE {
            return Err(CoreError::Chat(ChatFailureCode::Unknown));
        }
        let repository = self.repository.clone();
        let limit = request.limit;
        let (conversations, truncated) = blocking(move || repository.list_conversations(limit))
            .await?
            .map_err(persistence_failure)?;

        Ok(ListConversationsResponse {
            schema_version: CHAT_SCHEMA_VERSION,
            conversations: conversations
                .into_iter()
                .map(|conversation| ConversationSummary {
                    model: Self::model_identity(&conversation),
                    conversation_id: conversation.conversation_id,
                    title: conversation.title,
                    message_count: conversation.message_count,
                    created_at_unix_ms: conversation.created_at_unix_ms,
                    updated_at_unix_ms: conversation.updated_at_unix_ms,
                })
                .collect(),
            truncated,
            correlation_id: request.correlation_id,
            request_id: request.request_id,
        })
    }

    /// Returns one authoritative conversation snapshot.
    pub async fn get_conversation(
        &self,
        request: GetConversationRequest,
    ) -> Result<GetConversationResponse, CoreError> {
        Ok(GetConversationResponse {
            conversation: self.snapshot(request.conversation_id).await?,
            correlation_id: request.correlation_id,
            request_id: request.request_id,
        })
    }

    /// Returns provider-neutral evidence for the chat locality indicator.
    pub async fn runtime_status(
        &self,
        request: ChatRuntimeStatusRequest,
    ) -> Result<ChatRuntimeStatusResponse, CoreError> {
        let provider_id = self.provider.provider_id().clone();
        let context = RuntimeOperationContext::new(
            request.correlation_id,
            request.request_id,
            READINESS_TIMEOUT,
        );
        let report = match self.runtime.status(&provider_id, context.clone()).await {
            Ok(report) => report,
            Err(error) => {
                return Ok(ChatRuntimeStatusResponse {
                    status: blocked_runtime_status(
                        provider_id,
                        gixgiz_contracts::RuntimeDisplayName::new("Local runtime"),
                        runtime_chat_failure(&error),
                    ),
                    correlation_id: request.correlation_id,
                    request_id: request.request_id,
                });
            }
        };
        let display_name = report.display_name.clone();
        let blocked = if report.state == gixgiz_contracts::RuntimeState::Incompatible {
            Some(ChatFailureCode::RuntimeIncompatible)
        } else if !crate::runtime_is_usable(&report)
            || report.endpoint_safety != gixgiz_contracts::RuntimeEndpointSafety::LoopbackVerified
        {
            Some(ChatFailureCode::RuntimeUnavailable)
        } else {
            None
        };
        if let Some(code) = blocked {
            return Ok(ChatRuntimeStatusResponse {
                status: blocked_runtime_status(provider_id, display_name, code),
                correlation_id: request.correlation_id,
                request_id: request.request_id,
            });
        }

        let conversation = if let Some(conversation_id) = request.conversation_id {
            let repository = self.repository.clone();
            Some(
                blocking(move || repository.conversation(conversation_id))
                    .await?
                    .map_err(persistence_failure)?
                    .ok_or(CoreError::Chat(ChatFailureCode::ConversationNotFound))?,
            )
        } else {
            None
        };
        let inventory = match self.runtime.list_models(&provider_id, 100, context).await {
            Ok(inventory) => inventory,
            Err(error) => {
                return Ok(ChatRuntimeStatusResponse {
                    status: blocked_runtime_status(
                        provider_id,
                        display_name,
                        runtime_chat_failure(&error),
                    ),
                    correlation_id: request.correlation_id,
                    request_id: request.request_id,
                });
            }
        };
        let bound_model = conversation
            .as_ref()
            .and_then(|value| value.canonical_model_id.as_deref())
            .map(CandidateModelId::new);
        let available = match bound_model.as_ref() {
            Some(candidate) => inventory
                .models
                .iter()
                .find(|model| model.mapping.catalogue_id.as_ref() == Some(candidate)),
            None => inventory
                .models
                .iter()
                .find(|model| model.mapping.catalogue_id.is_some()),
        };
        let Some(available) = available else {
            let code = if bound_model.is_some() {
                ChatFailureCode::ModelChanged
            } else {
                ChatFailureCode::ModelUnavailable
            };
            return Ok(ChatRuntimeStatusResponse {
                status: blocked_runtime_status(provider_id, display_name, code),
                correlation_id: request.correlation_id,
                request_id: request.request_id,
            });
        };
        let Some(candidate) = available.mapping.catalogue_id.clone() else {
            return Ok(ChatRuntimeStatusResponse {
                status: blocked_runtime_status(
                    provider_id,
                    display_name,
                    ChatFailureCode::ModelUnavailable,
                ),
                correlation_id: request.correlation_id,
                request_id: request.request_id,
            });
        };
        let model = conversation
            .as_ref()
            .and_then(Self::model_identity)
            .unwrap_or_else(|| ChatModelIdentity {
                canonical_model_id: candidate,
                display_name: available.display_name.clone(),
                family: String::new(),
            });

        Ok(ChatRuntimeStatusResponse {
            status: ChatRuntimeStatus {
                schema_version: CHAT_SCHEMA_VERSION,
                provider_id,
                runtime_display_name: display_name,
                model: Some(model),
                locality: ChatLocalityStatus::RunningLocally,
                ready: true,
                blocked_by: None,
                recovery_action: None,
            },
            correlation_id: request.correlation_id,
            request_id: request.request_id,
        })
    }

    /// Replaces one conversation title.
    pub async fn rename_conversation(
        &self,
        request: RenameConversationRequest,
    ) -> Result<RenameConversationResponse, CoreError> {
        let title = normalize_title(&request.title)?;
        let repository = self.repository.clone();
        let conversation_id = request.conversation_id;
        let now = unix_millis();
        blocking(move || repository.rename_conversation(conversation_id, &title, now))
            .await?
            .map_err(persistence_failure)?;

        Ok(RenameConversationResponse {
            conversation: self.snapshot(conversation_id).await?,
            correlation_id: request.correlation_id,
            request_id: request.request_id,
        })
    }

    /// Deletes one conversation and the messages it owns.
    pub async fn delete_conversation(
        &self,
        request: DeleteConversationRequest,
    ) -> Result<DeleteConversationResponse, CoreError> {
        let repository = self.repository.clone();
        let conversation_id = request.conversation_id;
        let deleted_message_count =
            blocking(move || repository.delete_conversation(conversation_id))
                .await?
                .map_err(admission_failure)?;

        Ok(DeleteConversationResponse {
            conversation_id,
            deleted_message_count,
            deleted: true,
            correlation_id: request.correlation_id,
            request_id: request.request_id,
        })
    }

    /// Persists one user message and admits a bounded assistant generation.
    pub async fn send_message(
        &self,
        request: SendMessageRequest,
    ) -> Result<SendMessageResponse, CoreError> {
        let content = request.content.trim().to_owned();
        if content.is_empty() {
            return Err(CoreError::Chat(ChatFailureCode::Unknown));
        }
        if content.len() > MAX_USER_MESSAGE_BYTES {
            return Err(CoreError::Chat(ChatFailureCode::MessageTooLarge));
        }

        let conversation_id = request.conversation_id;
        let model = self
            .verify_ready_model(conversation_id, request.correlation_id, request.request_id)
            .await?;

        let history = self.history(conversation_id).await?;
        let BoundedContext { messages, warnings } = context::build_with_user(&history, &content)
            .map_err(|()| CoreError::Chat(ChatFailureCode::ContextTooLarge))?;
        let context_bytes: usize = messages.iter().map(|message| message.content.len()).sum();
        if context_bytes > MAX_CONTEXT_BYTES {
            return Err(CoreError::Chat(ChatFailureCode::ContextTooLarge));
        }

        let generation_id = GenerationId::new();
        let cancellation = RuntimeCancellationToken::new();
        if !self
            .generations
            .register(generation_id, cancellation.clone())
        {
            return Err(CoreError::Chat(ChatFailureCode::GenerationAlreadyActive));
        }

        let repository = self.repository.clone();
        let admission = blocking(move || {
            repository.admit_generation(
                conversation_id,
                MessageId::new(),
                MessageId::new(),
                generation_id,
                &content,
                unix_millis(),
            )
        })
        .await
        .and_then(|result| result.map_err(admission_failure));
        let admission = match admission {
            Ok(admission) => admission,
            Err(error) => {
                self.generations.release(generation_id);
                return Err(error);
            }
        };
        tracing::debug!(
            conversation_id = %conversation_id,
            generation_id = %generation_id,
            context_messages = messages.len(),
            context_bytes,
            "chat generation admitted"
        );

        self.spawn_generation(GenerationTask {
            generation_id,
            conversation_id,
            assistant_message_id: admission.assistant_message.message_id,
            correlation_id: request.correlation_id,
            request_id: request.request_id,
            canonical_model_id: model,
            messages,
            cancellation,
        });

        Ok(SendMessageResponse {
            schema_version: CHAT_SCHEMA_VERSION,
            user_message: contract_message(admission.user_message),
            assistant_message: contract_message(admission.assistant_message),
            generation_id,
            warnings,
            correlation_id: request.correlation_id,
            request_id: request.request_id,
        })
    }

    /// Returns a bounded ordered generation-event page.
    pub async fn events_after(
        &self,
        request: ChatGenerationEventsRequest,
    ) -> Result<ChatGenerationEventsResponse, CoreError> {
        if request.limit == 0 || request.limit > MAX_GENERATION_EVENT_PAGE {
            return Err(CoreError::Chat(ChatFailureCode::Unknown));
        }
        let Some(subscription) = self
            .generations
            .subscribe(request.generation_id, request.after_sequence)
        else {
            return Err(CoreError::Chat(ChatFailureCode::GenerationNotFound));
        };
        let mut events = subscription.replay;
        events.truncate(usize::from(request.limit));

        Ok(ChatGenerationEventsResponse {
            schema_version: CHAT_SCHEMA_VERSION,
            events,
            replay_incomplete: subscription.replay_incomplete,
            correlation_id: request.correlation_id,
            request_id: request.request_id,
        })
    }

    /// Requests cooperative cancellation of one active generation.
    pub async fn cancel_generation(
        &self,
        request: CancelGenerationRequest,
    ) -> Result<CancelGenerationResponse, CoreError> {
        let repository = self.repository.clone();
        let generation_id = request.generation_id;
        let now = unix_millis();
        let recorded = blocking(move || repository.request_cancellation(generation_id, now))
            .await?
            .map_err(persistence_failure)?;
        let signalled = self.generations.cancel(generation_id);

        Ok(CancelGenerationResponse {
            generation_id,
            accepted: recorded || signalled,
            correlation_id: request.correlation_id,
            request_id: request.request_id,
        })
    }

    /// Attaches one live observer strictly after the supplied cursor.
    ///
    /// The bounded replay window is a convenience only. When
    /// `replay_incomplete` is set the caller must reload the authoritative
    /// conversation snapshot rather than reconstruct state from events.
    pub fn observe(
        &self,
        generation_id: GenerationId,
        after_sequence: u64,
    ) -> Result<ChatEventSubscription, CoreError> {
        let subscription = self
            .generations
            .subscribe(generation_id, after_sequence)
            .ok_or(CoreError::Chat(ChatFailureCode::GenerationNotFound))?;
        Ok(ChatEventSubscription {
            replay: subscription.replay,
            replay_incomplete: subscription.replay_incomplete,
            terminal: subscription.terminal,
            receiver: subscription.receiver,
        })
    }

    async fn verify_ready_model(
        &self,
        conversation_id: ConversationId,
        correlation_id: CorrelationId,
        request_id: RequestId,
    ) -> Result<CandidateModelId, CoreError> {
        let repository = self.repository.clone();
        let conversation = blocking(move || repository.conversation(conversation_id))
            .await?
            .map_err(persistence_failure)?
            .ok_or(CoreError::Chat(ChatFailureCode::ConversationNotFound))?;

        let context = RuntimeOperationContext::new(correlation_id, request_id, READINESS_TIMEOUT);
        let provider_id: RuntimeProviderId = self.provider.provider_id().clone();
        let report = self
            .runtime
            .status(&provider_id, context.clone())
            .await
            .map_err(runtime_failure)?;
        if !crate::runtime_is_usable(&report) {
            return Err(CoreError::Chat(ChatFailureCode::RuntimeUnavailable));
        }

        let inventory = self
            .runtime
            .list_models(&provider_id, 100, context)
            .await
            .map_err(runtime_failure)?;
        let available = |candidate: &CandidateModelId| {
            inventory
                .models
                .iter()
                .any(|model| model.mapping.catalogue_id.as_ref() == Some(candidate))
        };

        if let Some(existing) = conversation.canonical_model_id.map(CandidateModelId::new) {
            // A conversation keeps the exact model it was verified against.
            if !available(&existing) {
                return Err(CoreError::Chat(ChatFailureCode::ModelChanged));
            }
            return Ok(existing);
        }

        let resolved = inventory
            .models
            .iter()
            .find_map(|model| model.mapping.catalogue_id.clone())
            .ok_or(CoreError::Chat(ChatFailureCode::ModelUnavailable))?;
        let repository = self.repository.clone();
        let binding = resolved.as_str().to_owned();
        let now = unix_millis();
        // The conversations -> models foreign key means only a model the setup
        // workflow verified can be bound. Anything else is an unavailable model,
        // not a storage fault.
        blocking(move || repository.bind_model(conversation_id, &binding, now))
            .await?
            .map_err(|_| CoreError::Chat(ChatFailureCode::ModelUnavailable))?;
        Ok(resolved)
    }

    fn model_identity(
        conversation: &gixgiz_persistence::PersistedConversation,
    ) -> Option<ChatModelIdentity> {
        let canonical = conversation.canonical_model_id.as_deref()?;
        Some(ChatModelIdentity {
            canonical_model_id: CandidateModelId::new(canonical),
            display_name: conversation
                .model_display_name
                .clone()
                .unwrap_or_else(|| canonical.to_owned()),
            family: conversation.model_family.clone().unwrap_or_default(),
        })
    }

    async fn history(
        &self,
        conversation_id: ConversationId,
    ) -> Result<Vec<PersistedChatMessage>, CoreError> {
        let repository = self.repository.clone();
        blocking(move || repository.messages(conversation_id))
            .await?
            .map_err(persistence_failure)
    }

    async fn snapshot(
        &self,
        conversation_id: ConversationId,
    ) -> Result<ConversationSnapshot, CoreError> {
        let repository = self.repository.clone();
        let conversation = blocking(move || repository.conversation(conversation_id))
            .await?
            .map_err(persistence_failure)?
            .ok_or(CoreError::Chat(ChatFailureCode::ConversationNotFound))?;
        let messages = self.history(conversation_id).await?;

        let active_generation_id = messages
            .iter()
            .find(|message| message.status == PersistedChatMessageStatus::Generating)
            .and_then(|message| message.generation_id);
        let mut warnings = Vec::new();
        if messages.iter().any(|message| {
            message.failure_code.as_deref() == Some(gixgiz_persistence::INTERRUPTED_FAILURE_CODE)
        }) {
            warnings.push(ChatWarning {
                code: ChatWarningCode::InterruptedGenerationRecovered,
                message: "A reply was interrupted when GixGiz last closed and is incomplete."
                    .to_owned(),
            });
        }

        Ok(ConversationSnapshot {
            schema_version: CHAT_SCHEMA_VERSION,
            conversation_id,
            model: Self::model_identity(&conversation),
            title: conversation.title,
            messages: messages.into_iter().map(contract_message).collect(),
            active_generation_id,
            warnings,
            created_at_unix_ms: conversation.created_at_unix_ms,
            updated_at_unix_ms: conversation.updated_at_unix_ms,
        })
    }

    fn spawn_generation(&self, task: GenerationTask) {
        let span = tracing::info_span!(
            "chat_generation",
            conversation_id = %task.conversation_id,
            generation_id = %task.generation_id,
            correlation_id = %task.correlation_id,
            request_id = %task.request_id,
        );
        let service = self.clone();
        tokio::spawn(async move { service.run_generation(task).await }.instrument(span));
    }

    async fn run_generation(&self, task: GenerationTask) {
        let mut state = GenerationState::new(&task);
        self.emit(&mut state, ChatGenerationEventKind::Started, None, None);

        let (sender, mut receiver) = mpsc::channel(CHAT_DELTA_CHANNEL_CAPACITY);
        let context =
            RuntimeOperationContext::new(task.correlation_id, task.request_id, GENERATION_TIMEOUT)
                .with_cancellation(task.cancellation.clone());
        let request = RuntimeChatRequest {
            canonical_model_id: task.canonical_model_id.clone(),
            messages: task.messages.clone(),
            max_output_bytes: MAX_ASSISTANT_OUTPUT_BYTES,
        };

        let provider = self.provider.clone();
        let mut generation = Box::pin(provider.generate(request, sender, context));
        let outcome = loop {
            tokio::select! {
                delta = receiver.recv() => {
                    if let Some(delta) = delta {
                        state.accumulated.push_str(&delta.text);
                        self.emit(
                            &mut state,
                            ChatGenerationEventKind::Delta,
                            Some(delta.text),
                            None,
                        );
                        self.maybe_checkpoint(&mut state).await;
                    }
                }
                result = &mut generation => break result,
            }
        };
        while let Ok(delta) = receiver.try_recv() {
            state.accumulated.push_str(&delta.text);
            self.emit(
                &mut state,
                ChatGenerationEventKind::Delta,
                Some(delta.text),
                None,
            );
        }

        self.commit(state, outcome, &task).await;
        self.generations.release(task.generation_id);
    }

    async fn maybe_checkpoint(&self, state: &mut GenerationState) {
        if state.accumulated.len() < state.checkpointed_bytes + CHECKPOINT_BYTES {
            return;
        }
        let repository = self.repository.clone();
        let generation_id = state.generation_id;
        let content = state.accumulated.clone();
        let sequence = state.sequence;
        let now = unix_millis();
        if blocking(move || {
            repository.checkpoint_assistant_content(generation_id, &content, sequence, now)
        })
        .await
        .is_ok()
        {
            state.checkpointed_bytes = state.accumulated.len();
        }
    }

    async fn commit(
        &self,
        mut state: GenerationState,
        outcome: Result<gixgiz_runtime::RuntimeGenerationResult, RuntimeError>,
        task: &GenerationTask,
    ) {
        let cancelled = task.cancellation.is_cancelled();
        let (persisted, kind, terminal, failure_code) = match (&outcome, cancelled) {
            (_, true) | (Err(RuntimeError::Cancelled), _) => (
                PersistedGenerationOutcome::Cancelled,
                ChatGenerationEventKind::Cancelled,
                ChatGenerationTerminalState::Cancelled,
                None,
            ),
            (Ok(_), false) => (
                PersistedGenerationOutcome::Completed,
                ChatGenerationEventKind::Completed,
                ChatGenerationTerminalState::Completed,
                None,
            ),
            (Err(RuntimeError::TimedOut), false) => (
                PersistedGenerationOutcome::Failed,
                ChatGenerationEventKind::TimedOut,
                ChatGenerationTerminalState::TimedOut,
                Some("chat.generation_timed_out"),
            ),
            (Err(error), false) => (
                PersistedGenerationOutcome::Failed,
                ChatGenerationEventKind::Failed,
                ChatGenerationTerminalState::Failed,
                Some(runtime_failure_code(error)),
            ),
        };

        let repository = self.repository.clone();
        let generation_id = state.generation_id;
        let content = state.accumulated.clone();
        let sequence = state.sequence + 1;
        let now = unix_millis();
        let code = failure_code.map(ToOwned::to_owned);
        let committed = blocking(move || {
            repository.finish_assistant_generation(
                generation_id,
                persisted,
                &content,
                code.as_deref(),
                sequence,
                now,
            )
        })
        .await
        .and_then(|result| result.map_err(persistence_failure));

        if committed.is_ok() {
            let error = failure_code.map(|code| {
                gixgiz_contracts::SafeErrorPayload::new(
                    gixgiz_contracts::ErrorCategory::Degraded,
                    code,
                    "The local model stopped before finishing its reply.",
                    gixgiz_contracts::RecoveryGuidance {
                        action: gixgiz_contracts::RecoveryAction::Retry,
                        message: "Check the local runtime, then send the message again.".to_owned(),
                    },
                    task.correlation_id,
                    task.request_id,
                )
            });
            tracing::info!(
                terminal_state = ?terminal,
                failure_code = failure_code.unwrap_or("none"),
                output_bytes = state.accumulated.len(),
                "chat generation terminal state committed"
            );
            self.emit(&mut state, kind, None, Some((terminal, error)));
        } else {
            tracing::warn!(
                error_code = "chat.persistence_unavailable",
                "chat terminal state was not committed"
            );
            let context = crate::OperationContext::new(task.correlation_id, task.request_id);
            let error =
                CoreError::Chat(ChatFailureCode::PersistenceUnavailable).to_safe_payload(&context);
            self.emit_durability_interrupted(&mut state, error);
        }
    }

    fn emit_durability_interrupted(
        &self,
        state: &mut GenerationState,
        error: gixgiz_contracts::SafeErrorPayload,
    ) {
        state.sequence += 1;
        self.generations.publish(ChatGenerationEvent {
            schema_version: CHAT_SCHEMA_VERSION,
            generation_id: state.generation_id,
            conversation_id: state.conversation_id,
            assistant_message_id: state.assistant_message_id,
            correlation_id: state.correlation_id,
            sequence: state.sequence,
            kind: ChatGenerationEventKind::DurabilityInterrupted,
            delta: None,
            terminal_state: None,
            error: Some(error),
            occurred_at_unix_ms: unix_millis(),
        });
    }

    fn emit(
        &self,
        state: &mut GenerationState,
        kind: ChatGenerationEventKind,
        delta: Option<String>,
        terminal: Option<(
            ChatGenerationTerminalState,
            Option<gixgiz_contracts::SafeErrorPayload>,
        )>,
    ) {
        state.sequence += 1;
        let (terminal_state, error) =
            terminal.map_or((None, None), |(state, error)| (Some(state), error));
        self.generations.publish(ChatGenerationEvent {
            schema_version: CHAT_SCHEMA_VERSION,
            generation_id: state.generation_id,
            conversation_id: state.conversation_id,
            assistant_message_id: state.assistant_message_id,
            correlation_id: state.correlation_id,
            sequence: state.sequence,
            kind,
            delta,
            terminal_state,
            error,
            occurred_at_unix_ms: unix_millis(),
        });
    }
}

/// Live view of one generation attached by a host observer.
pub struct ChatEventSubscription {
    /// Retained events strictly after the requested cursor.
    pub replay: Vec<ChatGenerationEvent>,
    /// Whether bounded retention discarded events before the replay window.
    pub replay_incomplete: bool,
    /// Whether the generation already reached a terminal event.
    pub terminal: bool,
    /// Live events emitted after this subscription attached.
    pub receiver: tokio::sync::broadcast::Receiver<ChatGenerationEvent>,
}

struct GenerationTask {
    generation_id: GenerationId,
    conversation_id: ConversationId,
    assistant_message_id: MessageId,
    correlation_id: CorrelationId,
    request_id: RequestId,
    canonical_model_id: CandidateModelId,
    messages: Vec<gixgiz_runtime::RuntimeChatMessage>,
    cancellation: RuntimeCancellationToken,
}

struct GenerationState {
    generation_id: GenerationId,
    conversation_id: ConversationId,
    assistant_message_id: MessageId,
    correlation_id: CorrelationId,
    sequence: u64,
    accumulated: String,
    checkpointed_bytes: usize,
}

impl GenerationState {
    fn new(task: &GenerationTask) -> Self {
        Self {
            generation_id: task.generation_id,
            conversation_id: task.conversation_id,
            assistant_message_id: task.assistant_message_id,
            correlation_id: task.correlation_id,
            sequence: 0,
            accumulated: String::new(),
            checkpointed_bytes: 0,
        }
    }
}

async fn blocking<T, F>(operation: F) -> Result<T, CoreError>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|_| CoreError::Chat(ChatFailureCode::PersistenceUnavailable))
}

fn normalize_title(value: &str) -> Result<String, CoreError> {
    let collapsed: String = value
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    let trimmed = collapsed.trim();
    if trimmed.is_empty() {
        return Err(CoreError::Chat(ChatFailureCode::Unknown));
    }
    let mut title = String::new();
    for character in trimmed.chars() {
        if title.len() + character.len_utf8() > MAX_CONVERSATION_TITLE_BYTES {
            break;
        }
        title.push(character);
    }
    Ok(title)
}

fn contract_message(message: PersistedChatMessage) -> ChatMessage {
    ChatMessage {
        message_id: message.message_id,
        conversation_id: message.conversation_id,
        role: match message.role {
            PersistedChatRole::User => ChatRole::User,
            PersistedChatRole::Assistant => ChatRole::Assistant,
            _ => ChatRole::Unknown,
        },
        status: match message.status {
            PersistedChatMessageStatus::Pending => ChatMessageStatus::Pending,
            PersistedChatMessageStatus::Generating => ChatMessageStatus::Generating,
            PersistedChatMessageStatus::Completed => ChatMessageStatus::Completed,
            PersistedChatMessageStatus::Cancelled => ChatMessageStatus::Cancelled,
            PersistedChatMessageStatus::Failed => ChatMessageStatus::Failed,
            _ => ChatMessageStatus::Unknown,
        },
        sequence: message.sequence,
        content: message.content,
        generation_id: message.generation_id,
        last_event_sequence: message.last_event_sequence,
        created_at_unix_ms: message.created_at_unix_ms,
        updated_at_unix_ms: message.updated_at_unix_ms,
        completed_at_unix_ms: message.completed_at_unix_ms,
    }
}

const fn runtime_failure_code(error: &RuntimeError) -> &'static str {
    match error {
        RuntimeError::ModelUnavailable | RuntimeError::ModelNotMapped => "chat.model_unavailable",
        RuntimeError::ProviderUnavailable => "chat.provider_disconnected",
        RuntimeError::InvalidResponse => "chat.malformed_provider_stream",
        RuntimeError::OutputLimit => "chat.output_limit",
        _ => "chat.generation_failed",
    }
}

const fn runtime_chat_failure(error: &RuntimeError) -> ChatFailureCode {
    match error {
        RuntimeError::NotInstalled | RuntimeError::ProviderUnavailable => {
            ChatFailureCode::RuntimeUnavailable
        }
        RuntimeError::IncompatibleVersion => ChatFailureCode::RuntimeIncompatible,
        RuntimeError::ConsentRequired => ChatFailureCode::RuntimeConsentRequired,
        RuntimeError::ModelUnavailable | RuntimeError::ModelNotMapped => {
            ChatFailureCode::ModelUnavailable
        }
        RuntimeError::TimedOut => ChatFailureCode::GenerationTimedOut,
        RuntimeError::Cancelled => ChatFailureCode::Cancelled,
        _ => ChatFailureCode::RuntimeUnavailable,
    }
}

fn runtime_failure(error: RuntimeError) -> CoreError {
    CoreError::Chat(runtime_chat_failure(&error))
}

fn persistence_failure(_error: gixgiz_persistence::PersistenceError) -> CoreError {
    CoreError::Chat(ChatFailureCode::PersistenceUnavailable)
}

fn admission_failure(error: gixgiz_persistence::PersistenceError) -> CoreError {
    match error {
        gixgiz_persistence::PersistenceError::RecordConflict {
            entity: "chat_generation",
        } => CoreError::Chat(ChatFailureCode::GenerationAlreadyActive),
        other => persistence_failure(other),
    }
}

fn blocked_runtime_status(
    provider_id: RuntimeProviderId,
    runtime_display_name: gixgiz_contracts::RuntimeDisplayName,
    code: ChatFailureCode,
) -> ChatRuntimeStatus {
    ChatRuntimeStatus {
        schema_version: CHAT_SCHEMA_VERSION,
        provider_id,
        runtime_display_name,
        model: None,
        locality: ChatLocalityStatus::Unknown,
        ready: false,
        blocked_by: Some(code),
        recovery_action: Some(match code {
            ChatFailureCode::ModelUnavailable | ChatFailureCode::ModelChanged => {
                ChatRecoveryAction::RunModelSetup
            }
            _ => ChatRecoveryAction::CheckRuntime,
        }),
    }
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}
