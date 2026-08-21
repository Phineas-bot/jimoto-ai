import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gixgiz_desktop/app/app_keys.dart';
import 'package:gixgiz_desktop/core/core_client.dart';
import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';
import 'package:gixgiz_desktop/features/chat/chat_page.dart';
import 'package:gixgiz_desktop/l10n/app_localizations.dart';

const _conversationId = '11111111-1111-4111-8111-111111111111';
const _generationId = '22222222-2222-4222-8222-222222222222';
const _correlationId = '33333333-3333-4333-8333-333333333333';
const _requestId = '44444444-4444-4444-8444-444444444444';
const _readyRuntimeStatus = ChatRuntimeStatus(
  schemaVersion: 1,
  providerId: 'gixgiz.runtime.test.v1',
  runtimeDisplayName: 'Test local runtime',
  locality: ChatLocalityStatus.runningLocally,
  ready: true,
  model: ChatModelIdentity(
    canonicalModelId: 'qwen2.5.0.5b-instruct',
    displayName: 'Qwen 2.5 Compact',
    family: 'Qwen 2.5',
  ),
  blockedBy: null,
  recoveryAction: null,
);

ChatMessage _message({
  required String id,
  required ChatRole role,
  required ChatMessageStatus status,
  required int sequence,
  required String content,
  int lastEventSequence = 0,
}) {
  return ChatMessage(
    messageId: id,
    conversationId: _conversationId,
    role: role,
    status: status,
    sequence: sequence,
    content: content,
    generationId: role == ChatRole.assistant ? _generationId : null,
    lastEventSequence: lastEventSequence,
    createdAtUnixMs: 1,
    updatedAtUnixMs: 1,
    completedAtUnixMs: status == ChatMessageStatus.generating ? null : 1,
  );
}

ConversationSnapshot _snapshot({
  List<ChatMessage> messages = const [],
  List<ChatWarning> warnings = const [],
  String title = 'First conversation',
  GenerationId? activeGenerationId,
}) {
  return ConversationSnapshot(
    schemaVersion: 1,
    conversationId: _conversationId,
    title: title,
    model: const ChatModelIdentity(
      canonicalModelId: 'qwen2.5.0.5b-instruct',
      displayName: 'Qwen 2.5 Compact',
      family: 'Qwen 2.5',
    ),
    messages: messages,
    activeGenerationId: activeGenerationId,
    warnings: warnings,
    createdAtUnixMs: 1,
    updatedAtUnixMs: 1,
  );
}

ChatGenerationEvent _event({
  required int sequence,
  String? delta,
  ChatGenerationTerminalState? terminal,
  ChatGenerationEventKind kind = ChatGenerationEventKind.delta,
}) {
  return ChatGenerationEvent(
    schemaVersion: 1,
    generationId: _generationId,
    conversationId: _conversationId,
    assistantMessageId: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
    correlationId: _correlationId,
    sequence: sequence,
    kind: kind,
    delta: delta,
    terminalState: terminal,
    error: null,
    occurredAtUnixMs: sequence,
  );
}

/// Deterministic in-memory client; performs no transport or provider work.
class _FakeChatClient extends CoreClient {
  _FakeChatClient({
    this.conversations = const [],
    ConversationSnapshot? snapshot,
    this.sendFailure,
    this.listFailure,
    ChatRuntimeStatus? runtimeStatus,
  }) : runtimeStatus = runtimeStatus ?? _readyRuntimeStatus,
       snapshot = snapshot ?? _snapshot();

  final List<ConversationSummary> conversations;
  ConversationSnapshot snapshot;
  final CoreClientFailure? sendFailure;
  final CoreClientFailure? listFailure;
  final ChatRuntimeStatus runtimeStatus;

  final StreamController<ChatGenerationEvent> controller =
      StreamController<ChatGenerationEvent>.broadcast();
  int createdCount = 0;
  int deletedCount = 0;
  int cancelledCount = 0;
  String? renamedTitle;
  String? sentContent;
  int? observedAfterSequence;

  @override
  Future<CoreConnectionSnapshot> checkConnection() async =>
      const CoreConnectionSnapshot(kind: CoreConnectionKind.ready);

  @override
  Future<ListConversationsResponse> listConversations({int limit = 50}) async {
    if (listFailure != null) {
      throw listFailure!;
    }
    return ListConversationsResponse(
      schemaVersion: 1,
      conversations: conversations,
      truncated: false,
      correlationId: _correlationId,
      requestId: _requestId,
    );
  }

  @override
  Future<ChatRuntimeStatus> chatRuntimeStatus({
    ConversationId? conversationId,
  }) async => runtimeStatus;

  @override
  Future<ConversationSnapshot> createConversation({String? title}) async {
    createdCount += 1;
    return snapshot;
  }

  @override
  Future<ConversationSnapshot> getConversation(
    ConversationId conversationId,
  ) async => snapshot;

  @override
  Future<ConversationSnapshot> renameConversation(
    ConversationId conversationId,
    String title,
  ) async {
    renamedTitle = title;
    snapshot = _snapshot(messages: snapshot.messages, title: title);
    return snapshot;
  }

  @override
  Future<DeleteConversationResponse> deleteConversation(
    ConversationId conversationId,
  ) async {
    deletedCount += 1;
    return DeleteConversationResponse(
      conversationId: conversationId,
      deletedMessageCount: 2,
      deleted: true,
      correlationId: _correlationId,
      requestId: _requestId,
    );
  }

  @override
  Future<SendMessageResponse> sendChatMessage(
    ConversationId conversationId,
    String content,
  ) async {
    if (sendFailure != null) {
      throw sendFailure!;
    }
    sentContent = content;
    return SendMessageResponse(
      schemaVersion: 1,
      userMessage: _message(
        id: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
        role: ChatRole.user,
        status: ChatMessageStatus.completed,
        sequence: 1,
        content: content,
      ),
      assistantMessage: _message(
        id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
        role: ChatRole.assistant,
        status: ChatMessageStatus.generating,
        sequence: 2,
        content: '',
      ),
      generationId: _generationId,
      warnings: const [],
      correlationId: _correlationId,
      requestId: _requestId,
    );
  }

  @override
  Stream<ChatGenerationEvent> observeGeneration(
    GenerationId generationId, {
    int afterSequence = 0,
  }) {
    observedAfterSequence = afterSequence;
    return controller.stream;
  }

  @override
  Future<bool> cancelGeneration(GenerationId generationId) async {
    cancelledCount += 1;
    return true;
  }
}

class _DelayedStatusChatClient extends _FakeChatClient {
  _DelayedStatusChatClient() : super(conversations: [_summary()]);

  final Completer<ChatRuntimeStatus> status = Completer<ChatRuntimeStatus>();

  @override
  Future<ChatRuntimeStatus> chatRuntimeStatus({
    ConversationId? conversationId,
  }) => status.future;
}

Widget _host(CoreClient client) {
  return MaterialApp(
    localizationsDelegates: AppLocalizations.localizationsDelegates,
    supportedLocales: AppLocalizations.supportedLocales,
    home: Scaffold(body: ChatPage(coreClient: client)),
  );
}

ConversationSummary _summary() => const ConversationSummary(
  conversationId: _conversationId,
  title: 'First conversation',
  messageCount: 0,
  model: null,
  createdAtUnixMs: 1,
  updatedAtUnixMs: 1,
);

void main() {
  testWidgets('empty conversation list invites starting one', (tester) async {
    await tester.pumpWidget(_host(_FakeChatClient()));
    await tester.pumpAndSettle();

    expect(find.byKey(AppKeys.chatConversationsEmpty), findsOneWidget);
    expect(find.byKey(AppKeys.chatNewConversationButton), findsOneWidget);
  });

  testWidgets('creating a conversation opens it', (tester) async {
    final client = _FakeChatClient();
    await tester.pumpWidget(_host(client));
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(AppKeys.chatNewConversationButton));
    await tester.pumpAndSettle();

    expect(client.createdCount, 1);
    expect(find.byKey(AppKeys.chatMessageList), findsOneWidget);
    expect(find.byKey(AppKeys.chatComposer), findsOneWidget);
  });

  testWidgets('locality stays unknown until core evidence arrives', (
    tester,
  ) async {
    final client = _DelayedStatusChatClient();
    await tester.pumpWidget(_host(client));
    await tester.pump();

    expect(find.text('Checking local runtime...'), findsOneWidget);
    expect(find.textContaining('Running locally'), findsNothing);

    client.status.complete(_readyRuntimeStatus);
    await tester.pumpAndSettle();
    expect(find.textContaining('Running locally'), findsOneWidget);
  });

  testWidgets('runtime loss replaces the confident locality statement', (
    tester,
  ) async {
    const unavailable = ChatRuntimeStatus(
      schemaVersion: 1,
      providerId: 'gixgiz.runtime.test.v1',
      runtimeDisplayName: 'Test local runtime',
      locality: ChatLocalityStatus.unknown,
      ready: false,
      model: null,
      blockedBy: ChatFailureCode.runtimeUnavailable,
      recoveryAction: ChatRecoveryAction.checkRuntime,
    );
    await tester.pumpWidget(
      _host(
        _FakeChatClient(
          conversations: [_summary()],
          runtimeStatus: unavailable,
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Local runtime needs attention'), findsOneWidget);
    expect(find.textContaining('Running locally'), findsNothing);
  });

  testWidgets('model loss is distinguished from runtime loss', (tester) async {
    const unavailable = ChatRuntimeStatus(
      schemaVersion: 1,
      providerId: 'gixgiz.runtime.test.v1',
      runtimeDisplayName: 'Test local runtime',
      locality: ChatLocalityStatus.unknown,
      ready: false,
      model: null,
      blockedBy: ChatFailureCode.modelUnavailable,
      recoveryAction: ChatRecoveryAction.runModelSetup,
    );
    await tester.pumpWidget(
      _host(
        _FakeChatClient(
          conversations: [_summary()],
          runtimeStatus: unavailable,
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Local model is not available'), findsOneWidget);
    expect(find.textContaining('Running locally'), findsNothing);
  });
  testWidgets('locality is stated without claiming the device is offline', (
    tester,
  ) async {
    final client = _FakeChatClient(conversations: [_summary()]);
    await tester.pumpWidget(_host(client));
    await tester.pumpAndSettle();
    await tester.tap(find.text('First conversation'));
    await tester.pumpAndSettle();

    expect(find.textContaining('Running locally'), findsOneWidget);
    expect(find.textContaining('Offline'), findsNothing);
    expect(find.textContaining('Qwen 2.5 Compact'), findsOneWidget);
  });

  testWidgets('sending shows a stop action and streams incrementally', (
    tester,
  ) async {
    final client = _FakeChatClient(conversations: [_summary()]);
    await tester.pumpWidget(_host(client));
    await tester.pumpAndSettle();
    await tester.tap(find.text('First conversation'));
    await tester.pumpAndSettle();

    await tester.enterText(find.byKey(AppKeys.chatComposer), 'hello');
    await tester.tap(find.byKey(AppKeys.chatSendButton));
    await tester.pumpAndSettle();

    expect(client.sentContent, 'hello');
    expect(find.byKey(AppKeys.chatStopButton), findsOneWidget);
    expect(find.byKey(AppKeys.chatSendButton), findsNothing);

    client.controller.add(_event(sequence: 1, delta: 'Hel'));
    await tester.pumpAndSettle();
    expect(find.textContaining('Hel'), findsOneWidget);

    client.controller.add(_event(sequence: 2, delta: 'lo there'));
    await tester.pumpAndSettle();
    expect(find.textContaining('Hello there'), findsOneWidget);
    expect(find.byKey(AppKeys.chatStreamingMessage), findsOneWidget);
  });

  testWidgets('stop requests cancellation for the active generation', (
    tester,
  ) async {
    final client = _FakeChatClient(conversations: [_summary()]);
    await tester.pumpWidget(_host(client));
    await tester.pumpAndSettle();
    await tester.tap(find.text('First conversation'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(AppKeys.chatComposer), 'hello');
    await tester.tap(find.byKey(AppKeys.chatSendButton));
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(AppKeys.chatStopButton));
    await tester.pumpAndSettle();

    expect(client.cancelledCount, 1);
  });

  testWidgets('active generation rehydrates from its durable cursor', (
    tester,
  ) async {
    final generating = _message(
      id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
      role: ChatRole.assistant,
      status: ChatMessageStatus.generating,
      sequence: 2,
      content: 'persisted partial',
      lastEventSequence: 7,
    );
    final client = _FakeChatClient(
      conversations: [_summary()],
      snapshot: _snapshot(
        messages: [generating],
        activeGenerationId: _generationId,
      ),
    );

    await tester.pumpWidget(_host(client));
    await tester.pumpAndSettle();
    await tester.tap(find.text('First conversation'));
    await tester.pumpAndSettle();

    expect(find.byKey(AppKeys.chatStopButton), findsOneWidget);
    expect(find.byKey(AppKeys.chatSendButton), findsNothing);
    expect(find.text('persisted partial'), findsOneWidget);
    expect(client.observedAfterSequence, 7);

    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pumpAndSettle();
    expect(client.cancelledCount, 0);

    await tester.pumpWidget(_host(client));
    await tester.pumpAndSettle();
    await tester.tap(find.text('First conversation'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(AppKeys.chatStopButton));
    await tester.pumpAndSettle();

    expect(client.observedAfterSequence, 7);
    expect(client.cancelledCount, 1);
  });

  testWidgets('active generation reloads the durable terminal snapshot', (
    tester,
  ) async {
    final client = _FakeChatClient(
      conversations: [_summary()],
      snapshot: _snapshot(
        messages: [
          _message(
            id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
            role: ChatRole.assistant,
            status: ChatMessageStatus.generating,
            sequence: 2,
            content: 'partial',
            lastEventSequence: 2,
          ),
        ],
        activeGenerationId: _generationId,
      ),
    );
    await tester.pumpWidget(_host(client));
    await tester.pumpAndSettle();
    await tester.tap(find.text('First conversation'));
    await tester.pumpAndSettle();

    client.snapshot = _snapshot(
      messages: [
        _message(
          id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
          role: ChatRole.assistant,
          status: ChatMessageStatus.completed,
          sequence: 2,
          content: 'final answer',
          lastEventSequence: 3,
        ),
      ],
    );
    client.controller.add(
      _event(
        sequence: 3,
        terminal: ChatGenerationTerminalState.completed,
        kind: ChatGenerationEventKind.completed,
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('final answer'), findsOneWidget);
    expect(find.byKey(AppKeys.chatSendButton), findsOneWidget);
    expect(find.byKey(AppKeys.chatStopButton), findsNothing);
  });

  testWidgets('active generation disables conversation deletion', (
    tester,
  ) async {
    final client = _FakeChatClient(
      conversations: [_summary()],
      snapshot: _snapshot(
        messages: [
          _message(
            id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
            role: ChatRole.assistant,
            status: ChatMessageStatus.generating,
            sequence: 2,
            content: '',
          ),
        ],
        activeGenerationId: _generationId,
      ),
    );
    await tester.pumpWidget(_host(client));
    await tester.pumpAndSettle();
    await tester.tap(find.text('First conversation'));
    await tester.pumpAndSettle();

    final delete = tester.widget<IconButton>(
      find.byKey(AppKeys.chatDeleteButton),
    );
    expect(delete.onPressed, isNull);
    expect(
      find.byTooltip('Stop the active reply before deleting this conversation'),
      findsOneWidget,
    );
    expect(client.deletedCount, 0);
  });
  testWidgets('a stopped reply is labelled incomplete from persisted state', (
    tester,
  ) async {
    final client = _FakeChatClient(
      conversations: [_summary()],
      snapshot: _snapshot(
        messages: [
          _message(
            id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
            role: ChatRole.assistant,
            status: ChatMessageStatus.cancelled,
            sequence: 1,
            content: 'half a rep',
          ),
        ],
      ),
    );
    await tester.pumpWidget(_host(client));
    await tester.pumpAndSettle();
    await tester.tap(find.text('First conversation'));
    await tester.pumpAndSettle();

    expect(find.textContaining('half a rep'), findsOneWidget);
    expect(find.textContaining('incomplete'), findsOneWidget);
  });

  testWidgets('runtime loss becomes an actionable attention state', (
    tester,
  ) async {
    final client = _FakeChatClient(
      conversations: [_summary()],
      sendFailure: const CoreClientFailure(
        code: 'chat.runtime_unavailable',
        category: ErrorCategory.unavailable,
        recoveryAction: RecoveryAction.checkPrerequisites,
        recoveryMessage: 'Check the local runtime from setup.',
      ),
    );
    await tester.pumpWidget(_host(client));
    await tester.pumpAndSettle();
    await tester.tap(find.text('First conversation'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(AppKeys.chatComposer), 'hello');
    await tester.tap(find.byKey(AppKeys.chatSendButton));
    await tester.pumpAndSettle();

    expect(find.byKey(AppKeys.chatAttention), findsOneWidget);
    expect(find.textContaining('chat.runtime_unavailable'), findsOneWidget);
    expect(find.text('Check the local runtime from setup.'), findsOneWidget);
    expect(find.byKey(AppKeys.chatRecoveryButton), findsOneWidget);
    expect(find.text('Return to setup'), findsOneWidget);
  });

  testWidgets('deletion requires explicit confirmation', (tester) async {
    final client = _FakeChatClient(conversations: [_summary()]);
    await tester.pumpWidget(_host(client));
    await tester.pumpAndSettle();
    await tester.tap(find.text('First conversation'));
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(AppKeys.chatDeleteButton));
    await tester.pumpAndSettle();
    expect(
      find.textContaining('does not remove the local runtime'),
      findsOneWidget,
    );
    expect(client.deletedCount, 0);

    await tester.tap(find.byKey(AppKeys.chatDeleteConfirmButton));
    await tester.pumpAndSettle();

    expect(client.deletedCount, 1);
  });

  testWidgets('a failing conversation list offers a retry', (tester) async {
    final client = _FakeChatClient(
      listFailure: const CoreClientFailure(
        code: 'chat.persistence_unavailable',
        category: ErrorCategory.internal,
        recoveryAction: RecoveryAction.restart,
      ),
    );
    await tester.pumpWidget(_host(client));
    await tester.pumpAndSettle();

    expect(find.byKey(AppKeys.chatRetryButton), findsOneWidget);
    expect(find.textContaining('chat.persistence_unavailable'), findsOneWidget);
  });

  testWidgets('streaming exposes a semantic live region and scales text', (
    tester,
  ) async {
    // Enlarged text needs a realistic desktop window, not the 800x600 default.
    tester.view.physicalSize = const Size(1400, 1000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    // Disposed inline: Flutter verifies handles before tearDown callbacks run.
    final handle = tester.ensureSemantics();
    final client = _FakeChatClient(conversations: [_summary()]);
    await tester.pumpWidget(
      MediaQuery(
        data: const MediaQueryData(textScaler: TextScaler.linear(1.8)),
        child: _host(client),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('First conversation'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(AppKeys.chatComposer), 'hello');
    await tester.tap(find.byKey(AppKeys.chatSendButton));
    await tester.pumpAndSettle();
    client.controller.add(_event(sequence: 1, delta: 'streamed'));
    await tester.pumpAndSettle();

    final semantics = tester.getSemantics(
      find.byKey(AppKeys.chatStreamingMessage),
    );
    expect(semantics.label, contains('replying'));
    expect(tester.takeException(), isNull);
    handle.dispose();
  });
}
