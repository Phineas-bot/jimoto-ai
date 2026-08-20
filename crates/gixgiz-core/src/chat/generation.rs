//! Bounded in-memory generation tracking.
//!
//! SQLite remains authoritative for conversation state across restarts. This
//! registry only holds live cancellation handles and a bounded replay window so
//! a briefly disconnected client can resume without reloading everything.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use gixgiz_contracts::{ChatGenerationEvent, GenerationId};
use gixgiz_runtime::RuntimeCancellationToken;
use tokio::sync::broadcast;

/// Largest number of events retained per generation for reconnect replay.
pub(super) const MAX_REPLAY_EVENTS: usize = 64;

/// Capacity of the live broadcast channel used by attached observers.
const BROADCAST_CAPACITY: usize = 64;

/// One live generation's cancellation handle and bounded replay window.
struct ActiveGeneration {
    cancellation: RuntimeCancellationToken,
    replay: Vec<ChatGenerationEvent>,
    dropped_events: bool,
    terminal: bool,
    sender: broadcast::Sender<ChatGenerationEvent>,
}

/// Live view attached by one observer.
pub(super) struct GenerationSubscription {
    /// Retained events strictly after the requested cursor.
    pub(super) replay: Vec<ChatGenerationEvent>,
    /// Whether bounded retention discarded events before the replay window.
    pub(super) replay_incomplete: bool,
    /// Whether the generation already reached a terminal event.
    pub(super) terminal: bool,
    /// Live events emitted after this subscription attached.
    pub(super) receiver: broadcast::Receiver<ChatGenerationEvent>,
}

/// Bounded registry of live generations.
#[derive(Clone, Default)]
pub(super) struct GenerationRegistry {
    active: Arc<Mutex<HashMap<GenerationId, ActiveGeneration>>>,
}

impl GenerationRegistry {
    /// Registers one admitted generation and its cancellation handle.
    pub(super) fn register(
        &self,
        generation_id: GenerationId,
        cancellation: RuntimeCancellationToken,
    ) -> bool {
        let Ok(mut active) = self.active.lock() else {
            return false;
        };
        if active.contains_key(&generation_id) {
            return false;
        }
        let (sender, _) = broadcast::channel(BROADCAST_CAPACITY);
        active.insert(
            generation_id,
            ActiveGeneration {
                cancellation,
                replay: Vec::new(),
                dropped_events: false,
                terminal: false,
                sender,
            },
        );
        true
    }

    /// Publishes one ordered event and retains it inside the bounded window.
    pub(super) fn publish(&self, event: ChatGenerationEvent) {
        let Ok(mut active) = self.active.lock() else {
            return;
        };
        let Some(entry) = active.get_mut(&event.generation_id) else {
            return;
        };
        if event.terminal_state.is_some() {
            entry.terminal = true;
        }
        if entry.replay.len() == MAX_REPLAY_EVENTS {
            entry.replay.remove(0);
            entry.dropped_events = true;
        }
        entry.replay.push(event.clone());
        // A closed channel only means nobody is currently attached.
        let _ = entry.sender.send(event);
    }

    /// Attaches one observer strictly after the supplied cursor.
    pub(super) fn subscribe(
        &self,
        generation_id: GenerationId,
        after_sequence: u64,
    ) -> Option<GenerationSubscription> {
        let active = self.active.lock().ok()?;
        let entry = active.get(&generation_id)?;
        let replay: Vec<ChatGenerationEvent> = entry
            .replay
            .iter()
            .filter(|event| event.sequence > after_sequence)
            .cloned()
            .collect();
        let earliest_retained = entry.replay.first().map_or(0, |event| event.sequence);
        let replay_incomplete = entry.dropped_events && after_sequence + 1 < earliest_retained;
        Some(GenerationSubscription {
            replay,
            replay_incomplete,
            terminal: entry.terminal,
            receiver: entry.sender.subscribe(),
        })
    }

    /// Requests cooperative cancellation of one live generation.
    pub(super) fn cancel(&self, generation_id: GenerationId) -> bool {
        let Ok(active) = self.active.lock() else {
            return false;
        };
        let Some(entry) = active.get(&generation_id) else {
            return false;
        };
        entry.cancellation.cancel();
        true
    }

    /// Releases one finished generation and its replay window.
    pub(super) fn release(&self, generation_id: GenerationId) {
        if let Ok(mut active) = self.active.lock() {
            active.remove(&generation_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use gixgiz_contracts::{
        CHAT_SCHEMA_VERSION, ChatGenerationEventKind, ChatGenerationTerminalState, ConversationId,
        CorrelationId, MessageId,
    };

    use super::*;

    fn event(generation_id: GenerationId, sequence: u64, terminal: bool) -> ChatGenerationEvent {
        ChatGenerationEvent {
            schema_version: CHAT_SCHEMA_VERSION,
            generation_id,
            conversation_id: ConversationId::new(),
            assistant_message_id: MessageId::new(),
            correlation_id: CorrelationId::new(),
            sequence,
            kind: if terminal {
                ChatGenerationEventKind::Completed
            } else {
                ChatGenerationEventKind::Delta
            },
            delta: (!terminal).then(|| "chunk".to_owned()),
            terminal_state: terminal.then_some(ChatGenerationTerminalState::Completed),
            error: None,
            occurred_at_unix_ms: sequence,
        }
    }

    #[test]
    fn replay_returns_only_events_after_the_cursor() {
        let registry = GenerationRegistry::default();
        let generation = GenerationId::new();
        registry.register(generation, RuntimeCancellationToken::new());
        for sequence in 1..=5 {
            registry.publish(event(generation, sequence, false));
        }

        let subscription = registry
            .subscribe(generation, 3)
            .expect("generation is active");
        let sequences: Vec<u64> = subscription
            .replay
            .iter()
            .map(|event| event.sequence)
            .collect();

        assert_eq!(sequences, vec![4, 5]);
        assert!(!subscription.replay_incomplete);
        assert!(!subscription.terminal);
    }

    #[test]
    fn retention_is_bounded_and_reports_an_incomplete_replay() {
        let registry = GenerationRegistry::default();
        let generation = GenerationId::new();
        registry.register(generation, RuntimeCancellationToken::new());
        for sequence in 1..=(MAX_REPLAY_EVENTS as u64 + 10) {
            registry.publish(event(generation, sequence, false));
        }

        let subscription = registry
            .subscribe(generation, 0)
            .expect("generation is active");

        assert_eq!(subscription.replay.len(), MAX_REPLAY_EVENTS);
        assert!(subscription.replay_incomplete);
    }

    #[test]
    fn terminal_events_are_visible_to_late_observers() {
        let registry = GenerationRegistry::default();
        let generation = GenerationId::new();
        registry.register(generation, RuntimeCancellationToken::new());
        registry.publish(event(generation, 1, false));
        registry.publish(event(generation, 2, true));

        let subscription = registry
            .subscribe(generation, 0)
            .expect("generation is active");

        assert!(subscription.terminal);
        assert_eq!(subscription.replay.len(), 2);
    }

    #[test]
    fn duplicate_registration_is_refused_and_release_frees_the_slot() {
        let registry = GenerationRegistry::default();
        let generation = GenerationId::new();

        assert!(registry.register(generation, RuntimeCancellationToken::new()));
        assert!(!registry.register(generation, RuntimeCancellationToken::new()));
        assert!(registry.subscribe(generation, 0).is_some());

        registry.release(generation);

        assert!(registry.subscribe(generation, 0).is_none());
        assert!(!registry.cancel(generation));
    }

    #[test]
    fn cancellation_reaches_the_registered_token() {
        let registry = GenerationRegistry::default();
        let generation = GenerationId::new();
        let token = RuntimeCancellationToken::new();
        registry.register(generation, token.clone());

        assert!(registry.cancel(generation));
        assert!(token.is_cancelled());
    }
}
