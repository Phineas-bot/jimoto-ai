import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:gixgiz_desktop/core/core_client.dart';
import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';
import 'package:gixgiz_desktop/features/chat/chat_screen.dart';
import 'package:gixgiz_desktop/features/chat/chat_state.dart';

/// Owns chat interaction state and talks to the core through [CoreClient] only.
///
/// This widget never calls a provider, never opens storage, and never infers
/// completion: every terminal state comes from a persisted core snapshot.
class ChatPage extends StatefulWidget {
  const ChatPage({required this.coreClient, super.key});

  final CoreClient coreClient;

  @override
  State<ChatPage> createState() => _ChatPageState();
}

class _ChatPageState extends State<ChatPage> {
  ChatViewState _state = const ChatViewState.initial();
  StreamSubscription<ChatGenerationEvent>? _generation;
  CoreClient? _streamClient;

  @override
  void initState() {
    super.initState();
    unawaited(_loadConversations());
  }

  @override
  void didUpdateWidget(covariant ChatPage oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(oldWidget.coreClient, widget.coreClient)) {
      _detachGeneration();
      setState(() => _state = const ChatViewState.initial());
      unawaited(_loadConversations());
    }
  }

  @override
  void dispose() {
    // Detaching never cancels the generation; core state stays authoritative.
    _detachGeneration();
    super.dispose();
  }

  void _detachGeneration() {
    unawaited(_generation?.cancel());
    _generation = null;
    _streamClient = null;
  }

  bool _isCurrent(CoreClient client) =>
      mounted && identical(client, widget.coreClient);

  Future<void> _loadConversations() async {
    final client = widget.coreClient;
    setState(() {
      _state = _state.copyWith(conversations: const ConversationListLoading());
    });
    try {
      final page = await client.listConversations();
      if (!_isCurrent(client)) {
        return;
      }
      setState(() {
        _state = _state.copyWith(
          conversations: page.conversations.isEmpty
              ? const ConversationListEmpty()
              : ConversationListReady(
                  conversations: page.conversations,
                  truncated: page.truncated,
                ),
        );
      });
    } on CoreClientFailure catch (failure) {
      if (!_isCurrent(client)) {
        return;
      }
      setState(() {
        _state = _state.copyWith(
          conversations: ConversationListFailed(diagnosticCode: failure.code),
        );
      });
    } on UnsupportedError {
      if (!_isCurrent(client)) {
        return;
      }
      setState(() {
        _state = _state.copyWith(
          conversations: const ConversationListFailed(
            diagnosticCode: 'CHAT_UNAVAILABLE',
          ),
        );
      });
    }
  }

  Future<void> _createConversation() async {
    final client = widget.coreClient;
    setState(() => _state = _state.copyWith(busy: true));
    try {
      final conversation = await client.createConversation();
      if (!_isCurrent(client)) {
        return;
      }
      setState(() {
        _state = _state.copyWith(
          busy: false,
          selectedConversationId: conversation.conversationId,
          conversation: ConversationReady(snapshot: conversation),
        );
      });
      await _loadConversations();
    } on CoreClientFailure catch (failure) {
      _reportConversationFailure(client, failure);
    }
  }

  Future<void> _selectConversation(ConversationId conversationId) async {
    _detachGeneration();
    setState(() {
      _state = _state.copyWith(
        selectedConversationId: conversationId,
        conversation: const ConversationLoading(),
      );
    });
    await _reloadConversation(conversationId);
  }

  Future<void> _reloadConversation(ConversationId conversationId) async {
    final client = widget.coreClient;
    try {
      final snapshot = await client.getConversation(conversationId);
      if (!_isCurrent(client)) {
        return;
      }
      setState(() {
        _state = _state.copyWith(
          conversation: ConversationReady(snapshot: snapshot),
        );
      });
    } on CoreClientFailure catch (failure) {
      _reportConversationFailure(client, failure);
    }
  }

  Future<void> _rename(ConversationId conversationId, String title) async {
    final client = widget.coreClient;
    setState(() => _state = _state.copyWith(busy: true));
    try {
      final snapshot = await client.renameConversation(conversationId, title);
      if (!_isCurrent(client)) {
        return;
      }
      setState(() {
        _state = _state.copyWith(
          busy: false,
          conversation: ConversationReady(snapshot: snapshot),
        );
      });
      await _loadConversations();
    } on CoreClientFailure catch (failure) {
      _reportConversationFailure(client, failure);
    }
  }

  Future<void> _delete(ConversationId conversationId) async {
    final client = widget.coreClient;
    _detachGeneration();
    setState(() => _state = _state.copyWith(busy: true));
    try {
      await client.deleteConversation(conversationId);
      if (!_isCurrent(client)) {
        return;
      }
      setState(() {
        _state = _state.copyWith(
          busy: false,
          clearSelection: true,
          conversation: const ConversationUnselected(),
        );
      });
      await _loadConversations();
    } on CoreClientFailure catch (failure) {
      _reportConversationFailure(client, failure);
    }
  }

  Future<void> _send(String content) async {
    final conversationId = _state.selectedConversationId;
    if (conversationId == null || !_state.canSend) {
      return;
    }
    final client = widget.coreClient;
    try {
      final sent = await client.sendChatMessage(conversationId, content);
      if (!_isCurrent(client)) {
        return;
      }
      final snapshot = await client.getConversation(conversationId);
      if (!_isCurrent(client)) {
        return;
      }
      setState(() {
        _state = _state.copyWith(
          conversation: ConversationGenerating(
            snapshot: snapshot,
            generationId: sent.generationId,
            streamingText: '',
          ),
        );
      });
      _attachGeneration(client, conversationId, sent.generationId);
    } on CoreClientFailure catch (failure) {
      _reportConversationFailure(client, failure);
    }
  }

  void _attachGeneration(
    CoreClient client,
    ConversationId conversationId,
    GenerationId generationId,
  ) {
    _streamClient = client;
    final buffer = StringBuffer();
    _generation = client
        .observeGeneration(generationId)
        .listen(
          (event) {
            if (!_isCurrent(client) || !identical(client, _streamClient)) {
              return;
            }
            final delta = event.delta;
            if (delta != null) {
              buffer.write(delta);
            }
            final current = _state.conversation;
            if (event.terminalState == null) {
              if (current is ConversationGenerating) {
                setState(() {
                  _state = _state.copyWith(
                    conversation: ConversationGenerating(
                      snapshot: current.snapshot,
                      generationId: generationId,
                      streamingText: buffer.toString(),
                    ),
                  );
                });
              }
              return;
            }
            // Terminal: reload the authoritative snapshot rather than trusting
            // the accumulated stream.
            _detachGeneration();
            unawaited(_reloadConversation(conversationId));
          },
          onError: (Object error) {
            if (!_isCurrent(client)) {
              return;
            }
            _detachGeneration();
            unawaited(_reloadConversation(conversationId));
          },
          onDone: () {
            if (!_isCurrent(client)) {
              return;
            }
            _detachGeneration();
            unawaited(_reloadConversation(conversationId));
          },
        );
  }

  Future<void> _stop() async {
    final current = _state.conversation;
    if (current is! ConversationGenerating) {
      return;
    }
    final client = widget.coreClient;
    try {
      await client.cancelGeneration(current.generationId);
    } on CoreClientFailure catch (failure) {
      _reportConversationFailure(client, failure);
    }
  }

  void _reportConversationFailure(CoreClient client, CoreClientFailure failure) {
    if (!_isCurrent(client)) {
      return;
    }
    final snapshot = switch (_state.conversation) {
      ConversationReady(:final snapshot) => snapshot,
      ConversationGenerating(:final snapshot) => snapshot,
      ConversationCancelled(:final snapshot) => snapshot,
      ConversationAttention(:final snapshot) => snapshot,
      ConversationFailed(:final snapshot) => snapshot,
      _ => null,
    };
    final attention = failure.category == ErrorCategory.unavailable ||
        failure.category == ErrorCategory.permissionDenied ||
        failure.category == ErrorCategory.conflict ||
        failure.category == ErrorCategory.incompatibleVersion;
    setState(() {
      _state = _state.copyWith(
        busy: false,
        conversation: attention
            ? ConversationAttention(
                diagnosticCode: failure.code,
                recoveryAction: failure.recoveryAction,
                snapshot: snapshot,
              )
            : ConversationFailed(
                diagnosticCode: failure.code,
                snapshot: snapshot,
              ),
      );
    });
  }

  @override
  Widget build(BuildContext context) {
    return ChatScreen(
      state: _state,
      onRefreshConversations: _loadConversations,
      onCreateConversation: _createConversation,
      onSelectConversation: _selectConversation,
      onRenameConversation: _rename,
      onDeleteConversation: _delete,
      onSendMessage: _send,
      onStopGeneration: _stop,
    );
  }
}
