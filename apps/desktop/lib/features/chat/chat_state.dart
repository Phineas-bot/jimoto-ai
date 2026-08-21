import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';

/// Provider-neutral locality/readiness evidence for chat.
sealed class ChatLocalityState {
  const ChatLocalityState();
}

final class ChatLocalityChecking extends ChatLocalityState {
  const ChatLocalityChecking();
}

final class ChatLocalityAvailable extends ChatLocalityState {
  const ChatLocalityAvailable({required this.status});

  final ChatRuntimeStatus status;
}

final class ChatLocalityFailed extends ChatLocalityState {
  const ChatLocalityFailed({required this.diagnosticCode});

  final String diagnosticCode;
}

/// State of the conversation list panel.
sealed class ConversationListState {
  const ConversationListState();
}

final class ConversationListLoading extends ConversationListState {
  const ConversationListLoading();
}

final class ConversationListEmpty extends ConversationListState {
  const ConversationListEmpty();
}

final class ConversationListReady extends ConversationListState {
  const ConversationListReady({
    required this.conversations,
    required this.truncated,
  });

  final List<ConversationSummary> conversations;
  final bool truncated;
}

final class ConversationListFailed extends ConversationListState {
  const ConversationListFailed({required this.diagnosticCode});

  final String diagnosticCode;
}

/// State of the active conversation transcript.
sealed class ConversationState {
  const ConversationState();
}

/// No conversation is selected yet.
final class ConversationUnselected extends ConversationState {
  const ConversationUnselected();
}

final class ConversationLoading extends ConversationState {
  const ConversationLoading();
}

/// A conversation is open and no reply is in flight.
final class ConversationReady extends ConversationState {
  const ConversationReady({required this.snapshot});

  final ConversationSnapshot snapshot;
}

/// A reply is streaming; [streamingText] is incomplete by definition.
final class ConversationGenerating extends ConversationState {
  const ConversationGenerating({
    required this.snapshot,
    required this.generationId,
    required this.streamingText,
  });

  final ConversationSnapshot snapshot;
  final GenerationId generationId;
  final String streamingText;
}

/// SQLite reports an active generation whose live replay is being recovered.
final class ConversationRecovering extends ConversationState {
  const ConversationRecovering({
    required this.snapshot,
    required this.generationId,
  });

  final ConversationSnapshot snapshot;
  final GenerationId generationId;
}

final class ConversationCancelled extends ConversationState {
/// The last reply was stopped by the user; retained text is partial.
  const ConversationCancelled({required this.snapshot});

  final ConversationSnapshot snapshot;
}

/// The local runtime or model needs attention before chatting can continue.
final class ConversationAttention extends ConversationState {
  const ConversationAttention({
    required this.diagnosticCode,
    required this.recoveryAction,
    this.recoveryMessage,
    this.snapshot,
  });

  final String diagnosticCode;
  final RecoveryAction recoveryAction;
  final String? recoveryMessage;
  final ConversationSnapshot? snapshot;
}

final class ConversationFailed extends ConversationState {
  const ConversationFailed({
    required this.diagnosticCode,
    this.recoveryAction = RecoveryAction.noAction,
    this.recoveryMessage,
    this.snapshot,
  });

  final String diagnosticCode;
  final RecoveryAction recoveryAction;
  final String? recoveryMessage;
  final ConversationSnapshot? snapshot;
}

/// Everything the chat screen renders, derived only from core state.
class ChatViewState {
  const ChatViewState({
    required this.conversations,
    required this.conversation,
    this.locality = const ChatLocalityChecking(),
    required this.selectedConversationId,
    this.busy = false,
  });

  const ChatViewState.initial()
    : conversations = const ConversationListLoading(),
      conversation = const ConversationUnselected(),
      locality = const ChatLocalityChecking(),
      selectedConversationId = null,
      busy = false;

  final ConversationListState conversations;
  final ConversationState conversation;
  final ChatLocalityState locality;
  final ConversationId? selectedConversationId;

  /// A create, rename, or delete action is in flight.
  final bool busy;

  /// Whether the composer may accept a new message.
  bool get canSend =>
      !busy &&
      selectedConversationId != null &&
      (conversation is ConversationReady ||
          conversation is ConversationCancelled);

  /// Whether a reply is active and can still be targeted for cancellation.
  bool get canStop =>
      conversation is ConversationGenerating ||
      conversation is ConversationRecovering;

  ChatViewState copyWith({
    ConversationListState? conversations,
    ConversationState? conversation,
    ChatLocalityState? locality,
    ConversationId? selectedConversationId,
    bool clearSelection = false,
    bool? busy,
  }) {
    return ChatViewState(
      conversations: conversations ?? this.conversations,
      conversation: conversation ?? this.conversation,
      locality: locality ?? this.locality,
      selectedConversationId: clearSelection
          ? null
          : (selectedConversationId ?? this.selectedConversationId),
      busy: busy ?? this.busy,
    );
  }
}
