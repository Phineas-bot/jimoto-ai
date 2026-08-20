//! Provider-neutral streaming generation consumed by local chat.
//!
//! This surface is deliberately separate from the Task 10 readiness inference.
//! Readiness proves a model can answer at all and discards its output; chat
//! streams user-visible content and must remain cancellable throughout.

use gixgiz_contracts::CandidateModelId;
use tokio::sync::mpsc;

use crate::{RuntimeFuture, RuntimeOperationContext};

/// Recommended bounded capacity for the normalized generation delta channel.
///
/// Providers must not treat this channel as the terminal result. Durable
/// message state remains authoritative if a consumer falls behind.
pub const CHAT_DELTA_CHANNEL_CAPACITY: usize = 32;

/// Bounded sender used for provider-neutral assistant text increments.
pub type ChatDeltaSender = mpsc::Sender<RuntimeGenerationDelta>;

/// Author of one context message supplied to a provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RuntimeChatRole {
    /// Content written by the person using GixGiz.
    User,
    /// Content previously produced by the model.
    Assistant,
}

/// One bounded context message supplied to a provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeChatMessage {
    /// Message author.
    pub role: RuntimeChatRole,
    /// Bounded message text validated by core policy before this call.
    pub content: String,
}

/// Bounded provider-neutral generation request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeChatRequest {
    /// Canonical catalogue identity verified for this conversation.
    ///
    /// The adapter resolves this to its own allowlisted artifact. No provider
    /// tag crosses the provider-neutral boundary.
    pub canonical_model_id: CandidateModelId,
    /// Ordered bounded context, oldest first.
    pub messages: Vec<RuntimeChatMessage>,
    /// Hard cap on total emitted assistant bytes.
    pub max_output_bytes: usize,
}

/// One normalized increment of assistant text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeGenerationDelta {
    /// Bounded assistant text carrying no provider status or metadata.
    pub text: String,
    /// Total assistant bytes emitted by this generation so far.
    pub emitted_bytes: usize,
}

/// Successful terminal evidence for one bounded generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeGenerationResult {
    /// Total assistant bytes emitted before the terminal provider signal.
    pub emitted_bytes: usize,
    /// UTC Unix timestamp in milliseconds captured after terminal success.
    pub completed_at_unix_ms: u64,
}

/// Provider-neutral streaming chat generation.
pub trait RuntimeChatProvider: Send + Sync {
    /// Streams one bounded assistant generation for an exact verified model.
    ///
    /// The caller must consume `deltas` concurrently. Providers must emit only
    /// assistant text through it, must never carry provider status text, and
    /// must not rely on it to report the terminal outcome. Cancellation and
    /// deadlines arrive through `context` and must close the provider request.
    fn generate(
        &self,
        request: RuntimeChatRequest,
        deltas: ChatDeltaSender,
        context: RuntimeOperationContext,
    ) -> RuntimeFuture<'_, RuntimeGenerationResult>;
}
