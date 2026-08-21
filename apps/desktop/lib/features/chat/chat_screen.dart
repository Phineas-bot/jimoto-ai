import 'package:flutter/material.dart';
import 'package:gixgiz_desktop/app/app_keys.dart';
import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';
import 'package:gixgiz_desktop/features/chat/chat_state.dart';
import 'package:gixgiz_desktop/l10n/app_localizations.dart';
import 'package:gixgiz_desktop/shared/page_header.dart';

/// Renders chat state and emits user intentions. It holds no chat logic.
class ChatScreen extends StatelessWidget {
  const ChatScreen({
    required this.state,
    required this.onRefreshConversations,
    required this.onCreateConversation,
    required this.onSelectConversation,
    required this.onRenameConversation,
    required this.onDeleteConversation,
    required this.onSendMessage,
    required this.onRecover,
    required this.onStopGeneration,
    super.key,
  });

  final ChatViewState state;
  final Future<void> Function() onRefreshConversations;
  final Future<void> Function() onCreateConversation;
  final Future<void> Function(ConversationId conversationId)
  onSelectConversation;
  final Future<void> Function(ConversationId conversationId, String title)
  onRenameConversation;
  final Future<void> Function(ConversationId conversationId)
  onDeleteConversation;
  final Future<void> Function(String content) onSendMessage;
  final Future<void> Function(RecoveryAction action) onRecover;
  final Future<void> Function() onStopGeneration;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    return Padding(
      padding: const EdgeInsets.all(24),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          PageHeader(sectionTitle: localizations.chatTitle),
          const SizedBox(height: 8),
          Text(
            localizations.chatDescription,
            style: Theme.of(context).textTheme.bodyMedium,
          ),
          const SizedBox(height: 16),
          _LocalityBadge(state: state),
          const SizedBox(height: 16),
          Expanded(
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                SizedBox(
                  width: 260,
                  child: _ConversationList(
                    state: state,
                    onCreateConversation: onCreateConversation,
                    onSelectConversation: onSelectConversation,
                    onRefreshConversations: onRefreshConversations,
                  ),
                ),
                const SizedBox(width: 16),
                Expanded(
                  child: _ConversationPane(
                    state: state,
                    onRenameConversation: onRenameConversation,
                    onDeleteConversation: onDeleteConversation,
                    onSendMessage: onSendMessage,
                    onRecover: onRecover,
                    onStopGeneration: onStopGeneration,
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// States locality accurately: local execution, not a claim about the network.
class _LocalityBadge extends StatelessWidget {
  const _LocalityBadge({required this.state});

  final ChatViewState state;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    final theme = Theme.of(context);
    final (label, icon) = switch (state.locality) {
      ChatLocalityChecking() => (
        localizations.chatLocalityChecking,
        Icons.sync,
      ),
      ChatLocalityFailed() => (
        localizations.chatLocalityUnknown,
        Icons.help_outline,
      ),
      ChatLocalityAvailable(:final status)
          when status.ready &&
              status.locality == ChatLocalityStatus.runningLocally =>
        (
          status.model == null
              ? localizations.chatRunningLocally
              : localizations.chatRunningLocallyWithModel(
                  status.model!.displayName,
                ),
          Icons.home_outlined,
        ),
      ChatLocalityAvailable(:final status)
          when status.blockedBy == ChatFailureCode.modelUnavailable ||
              status.blockedBy == ChatFailureCode.modelChanged =>
        (localizations.chatModelUnavailable, Icons.inventory_2_outlined),
      ChatLocalityAvailable() => (
        localizations.chatRuntimeUnavailable,
        Icons.warning_amber_outlined,
      ),
    };

    return Semantics(
      key: AppKeys.chatLocalityBadge,
      label: localizations.chatLocalitySemanticLabel(label),
      container: true,
      child: ExcludeSemantics(
        child: Row(
          mainAxisAlignment: MainAxisAlignment.start,
          children: [
            Icon(icon, size: 18),
            const SizedBox(width: 8),
            Flexible(child: Text(label, style: theme.textTheme.bodyMedium)),
          ],
        ),
      ),
    );
  }
}

class _ConversationList extends StatelessWidget {
  const _ConversationList({
    required this.state,
    required this.onCreateConversation,
    required this.onSelectConversation,
    required this.onRefreshConversations,
  });

  final ChatViewState state;
  final Future<void> Function() onCreateConversation;
  final Future<void> Function(ConversationId conversationId)
  onSelectConversation;
  final Future<void> Function() onRefreshConversations;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        FilledButton.icon(
          key: AppKeys.chatNewConversationButton,
          onPressed: state.busy ? null : () => onCreateConversation(),
          icon: const Icon(Icons.add),
          label: Text(localizations.chatNewConversationAction),
        ),
        const SizedBox(height: 12),
        Expanded(child: _buildBody(context, localizations)),
      ],
    );
  }

  Widget _buildBody(BuildContext context, AppLocalizations localizations) {
    return switch (state.conversations) {
      ConversationListLoading() => Semantics(
        liveRegion: true,
        label: localizations.chatConversationsLoading,
        child: const Center(child: CircularProgressIndicator()),
      ),
      ConversationListEmpty() => Center(
        key: AppKeys.chatConversationsEmpty,
        child: Text(localizations.chatConversationsEmpty),
      ),
      ConversationListFailed(:final diagnosticCode) => _InlineProblem(
        message: localizations.chatConversationsFailed,
        diagnosticCode: diagnosticCode,
        onRetry: onRefreshConversations,
      ),
      ConversationListReady(:final conversations) => ListView.builder(
        key: AppKeys.chatConversationList,
        itemCount: conversations.length,
        itemBuilder: (context, index) {
          final conversation = conversations[index];
          final selected =
              conversation.conversationId == state.selectedConversationId;
          return ListTile(
            selected: selected,
            title: Text(conversation.title, maxLines: 2),
            subtitle: Text(
              localizations.chatMessageCount(conversation.messageCount),
            ),
            onTap: state.busy
                ? null
                : () => onSelectConversation(conversation.conversationId),
          );
        },
      ),
    };
  }
}

class _ConversationPane extends StatelessWidget {
  const _ConversationPane({
    required this.state,
    required this.onRenameConversation,
    required this.onDeleteConversation,
    required this.onSendMessage,
    required this.onRecover,
    required this.onStopGeneration,
  });

  final ChatViewState state;
  final Future<void> Function(ConversationId conversationId, String title)
  onRenameConversation;
  final Future<void> Function(ConversationId conversationId)
  onDeleteConversation;
  final Future<void> Function(String content) onSendMessage;
  final Future<void> Function(RecoveryAction action) onRecover;
  final Future<void> Function() onStopGeneration;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    return switch (state.conversation) {
      ConversationUnselected() => Center(
        key: AppKeys.chatNoConversationSelected,
        child: Text(localizations.chatNoConversationSelected),
      ),
      ConversationLoading() => Semantics(
        liveRegion: true,
        label: localizations.chatConversationLoading,
        child: const Center(child: CircularProgressIndicator()),
      ),
      ConversationReady(:final snapshot) => _Transcript(
        snapshot: snapshot,
        state: state,
        streamingText: null,
        onRenameConversation: onRenameConversation,
        onDeleteConversation: onDeleteConversation,
        onSendMessage: onSendMessage,
        onStopGeneration: onStopGeneration,
      ),
      ConversationGenerating(:final snapshot, :final streamingText) =>
        _Transcript(
          snapshot: snapshot,
          state: state,
          streamingText: streamingText,
          onRenameConversation: onRenameConversation,
          onDeleteConversation: onDeleteConversation,
          onSendMessage: onSendMessage,
          onStopGeneration: onStopGeneration,
        ),
      ConversationRecovering(:final snapshot) => _Transcript(
        snapshot: snapshot,
        state: state,
        streamingText: null,
        onRenameConversation: onRenameConversation,
        onDeleteConversation: onDeleteConversation,
        onSendMessage: onSendMessage,
        onStopGeneration: onStopGeneration,
      ),
      ConversationCancelled(:final snapshot) => _Transcript(
        snapshot: snapshot,
        state: state,
        streamingText: null,
        onRenameConversation: onRenameConversation,
        onDeleteConversation: onDeleteConversation,
        onSendMessage: onSendMessage,
        onStopGeneration: onStopGeneration,
      ),
      ConversationAttention(
        :final diagnosticCode,
        :final recoveryAction,
        :final recoveryMessage,
      ) =>
        _InlineProblem(
          key: AppKeys.chatAttention,
          message: localizations.chatAttentionMessage,
          diagnosticCode: diagnosticCode,
          recoveryMessage: recoveryMessage,
          recoveryAction: recoveryAction,
          onRecover: onRecover,
        ),
      ConversationFailed(
        :final diagnosticCode,
        :final recoveryAction,
        :final recoveryMessage,
      ) =>
        _InlineProblem(
          key: AppKeys.chatFailed,
          message: localizations.chatFailedMessage,
          diagnosticCode: diagnosticCode,
          recoveryMessage: recoveryMessage,
          recoveryAction: recoveryAction,
          onRecover: onRecover,
        ),
    };
  }
}

class _Transcript extends StatelessWidget {
  const _Transcript({
    required this.snapshot,
    required this.state,
    required this.streamingText,
    required this.onRenameConversation,
    required this.onDeleteConversation,
    required this.onSendMessage,
    required this.onStopGeneration,
  });

  final ConversationSnapshot snapshot;
  final ChatViewState state;
  final String? streamingText;
  final Future<void> Function(ConversationId conversationId, String title)
  onRenameConversation;
  final Future<void> Function(ConversationId conversationId)
  onDeleteConversation;
  final Future<void> Function(String content) onSendMessage;
  final Future<void> Function() onStopGeneration;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Expanded(
              child: Text(
                snapshot.title,
                style: Theme.of(context).textTheme.titleMedium,
                maxLines: 2,
              ),
            ),
            IconButton(
              key: AppKeys.chatRenameButton,
              tooltip: localizations.chatRenameAction,
              onPressed: state.busy || state.canStop
                  ? null
                  : () => _promptRename(context, localizations),
              icon: const Icon(Icons.edit_outlined),
            ),
            IconButton(
              key: AppKeys.chatDeleteButton,
              tooltip: state.canStop
                  ? localizations.chatDeleteActiveTooltip
                  : localizations.chatDeleteAction,
              onPressed: state.busy || state.canStop
                  ? null
                  : () => _confirmDelete(context, localizations),
              icon: const Icon(Icons.delete_outline),
            ),
          ],
        ),
        if (state.conversation is ConversationRecovering)
          Semantics(
            key: AppKeys.chatRecovering,
            liveRegion: true,
            label: localizations.chatRecoveringReply,
            child: Text(
              localizations.chatRecoveringReply,
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ),
        for (final warning in snapshot.warnings)
          Padding(
            padding: const EdgeInsets.only(top: 4),
            child: Text(
              warning.message,
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ),
        const Divider(),
        Expanded(
          child: ListView(
            key: AppKeys.chatMessageList,
            children: [
              for (final message in snapshot.messages)
                if (streamingText == null ||
                    message.status != ChatMessageStatus.generating)
                  _MessageBubble(message: message),
              if (streamingText != null) _StreamingBubble(text: streamingText!),
            ],
          ),
        ),
        const Divider(),
        _Composer(
          state: state,
          onSendMessage: onSendMessage,
          onStopGeneration: onStopGeneration,
        ),
      ],
    );
  }

  Future<void> _promptRename(
    BuildContext context,
    AppLocalizations localizations,
  ) async {
    final controller = TextEditingController(text: snapshot.title);
    final title = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(localizations.chatRenameAction),
        content: TextField(
          controller: controller,
          autofocus: true,
          maxLength: 120,
          decoration: InputDecoration(labelText: localizations.chatRenameLabel),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: Text(localizations.chatCancelAction),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(controller.text),
            child: Text(localizations.chatConfirmAction),
          ),
        ],
      ),
    );
    if (title != null && title.trim().isNotEmpty) {
      await onRenameConversation(snapshot.conversationId, title.trim());
    }
  }

  Future<void> _confirmDelete(
    BuildContext context,
    AppLocalizations localizations,
  ) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(localizations.chatDeleteAction),
        content: Text(localizations.chatDeleteConfirmation),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(localizations.chatCancelAction),
          ),
          FilledButton(
            key: AppKeys.chatDeleteConfirmButton,
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(localizations.chatDeleteAction),
          ),
        ],
      ),
    );
    if (confirmed ?? false) {
      await onDeleteConversation(snapshot.conversationId);
    }
  }
}

class _MessageBubble extends StatelessWidget {
  const _MessageBubble({required this.message});

  final ChatMessage message;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    final theme = Theme.of(context);
    final author = message.role == ChatRole.user
        ? localizations.chatRoleUser
        : localizations.chatRoleAssistant;
    final incomplete =
        message.status == ChatMessageStatus.cancelled ||
        message.status == ChatMessageStatus.failed;

    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(author, style: theme.textTheme.labelMedium),
          const SizedBox(height: 4),
          // Assistant output is untrusted text and is never rendered as markup.
          SelectableText(message.content),
          if (incomplete)
            Padding(
              padding: const EdgeInsets.only(top: 4),
              child: Text(
                message.status == ChatMessageStatus.cancelled
                    ? localizations.chatReplyStopped
                    : localizations.chatReplyIncomplete,
                style: theme.textTheme.bodySmall,
              ),
            ),
        ],
      ),
    );
  }
}

class _StreamingBubble extends StatelessWidget {
  const _StreamingBubble({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 8),
      child: Semantics(
        key: AppKeys.chatStreamingMessage,
        liveRegion: true,
        label: localizations.chatReplyStreamingSemanticLabel,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              localizations.chatRoleAssistant,
              style: theme.textTheme.labelMedium,
            ),
            const SizedBox(height: 4),
            SelectableText(text),
            const SizedBox(height: 4),
            Text(
              localizations.chatReplyStreaming,
              style: theme.textTheme.bodySmall,
            ),
          ],
        ),
      ),
    );
  }
}

class _Composer extends StatefulWidget {
  const _Composer({
    required this.state,
    required this.onSendMessage,
    required this.onStopGeneration,
  });

  final ChatViewState state;
  final Future<void> Function(String content) onSendMessage;
  final Future<void> Function() onStopGeneration;

  @override
  State<_Composer> createState() => _ComposerState();
}

class _ComposerState extends State<_Composer> {
  final TextEditingController _controller = TextEditingController();

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    final content = _controller.text.trim();
    if (content.isEmpty || !widget.state.canSend) {
      return;
    }
    _controller.clear();
    await widget.onSendMessage(content);
  }

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    return Padding(
      padding: const EdgeInsets.only(top: 8),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          Expanded(
            child: TextField(
              key: AppKeys.chatComposer,
              controller: _controller,
              enabled: widget.state.canSend,
              maxLines: 4,
              minLines: 1,
              textInputAction: TextInputAction.send,
              onSubmitted: (_) => _submit(),
              decoration: InputDecoration(
                labelText: localizations.chatComposerLabel,
                hintText: localizations.chatComposerHint,
              ),
            ),
          ),
          const SizedBox(width: 12),
          if (widget.state.canStop)
            FilledButton.icon(
              key: AppKeys.chatStopButton,
              onPressed: () => widget.onStopGeneration(),
              icon: const Icon(Icons.stop),
              label: Text(localizations.chatStopAction),
            )
          else
            FilledButton.icon(
              key: AppKeys.chatSendButton,
              onPressed: widget.state.canSend ? () => _submit() : null,
              icon: const Icon(Icons.send),
              label: Text(localizations.chatSendAction),
            ),
        ],
      ),
    );
  }
}

class _InlineProblem extends StatelessWidget {
  const _InlineProblem({
    required this.message,
    required this.diagnosticCode,
    this.recoveryMessage,
    this.recoveryAction,
    this.onRecover,
    this.onRetry,
    super.key,
  });

  final String message;
  final String diagnosticCode;
  final String? recoveryMessage;
  final RecoveryAction? recoveryAction;
  final Future<void> Function(RecoveryAction action)? onRecover;
  final Future<void> Function()? onRetry;

  @override
  Widget build(BuildContext context) {
    final localizations = AppLocalizations.of(context);
    final actionLabel = _actionLabel(localizations, recoveryAction);
    final theme = Theme.of(context);
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Semantics(
            liveRegion: true,
            label: message,
            child: ExcludeSemantics(
              child: Text(message, textAlign: TextAlign.center),
            ),
          ),
          if (recoveryMessage != null &&
              recoveryMessage!.trim().isNotEmpty) ...[
            const SizedBox(height: 8),
            Text(recoveryMessage!, textAlign: TextAlign.center),
          ],
          const SizedBox(height: 8),
          Text(
            localizations.diagnosticCodeLabel(diagnosticCode),
            style: theme.textTheme.bodySmall,
          ),
          if (onRetry != null) ...[
            const SizedBox(height: 12),
            OutlinedButton(
              key: AppKeys.chatRetryButton,
              onPressed: () => onRetry!(),
              child: Text(localizations.chatRetryAction),
            ),
          ],
          if (actionLabel != null && onRecover != null) ...[
            const SizedBox(height: 12),
            OutlinedButton(
              key: AppKeys.chatRecoveryButton,
              onPressed: () => onRecover!(recoveryAction!),
              child: Text(actionLabel),
            ),
          ],
        ],
      ),
    );
  }
}

String? _actionLabel(
  AppLocalizations localizations,
  RecoveryAction? recoveryAction,
) {
  return switch (recoveryAction) {
    RecoveryAction.retry => localizations.chatRetryAction,
    RecoveryAction.checkPrerequisites => localizations.chatReturnToSetupAction,
    _ => null,
  };
}
