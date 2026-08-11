import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gixgiz_desktop/app/app_keys.dart';
import 'package:gixgiz_desktop/core/core_client.dart';
import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';
import 'package:gixgiz_desktop/features/foundation/foundation_page.dart';
import 'package:gixgiz_desktop/l10n/app_localizations.dart';

void main() {
  testWidgets('loads authoritative runtime status after core readiness', (
    tester,
  ) async {
    final client = _FakeRuntimeCoreClient(_runtimeReport(state: 'ready'));
    addTearDown(client.dispose);

    await _pumpPage(tester, client);

    expect(client.statusChecks, 1);
    expect(find.text('Runtime ready'), findsOneWidget);
  });

  testWidgets('does not query runtime status without negotiated capability', (
    tester,
  ) async {
    final client = _FakeRuntimeCoreClient(
      _runtimeReport(state: 'ready'),
      supportsRuntimeStatus: false,
    );
    addTearDown(client.dispose);

    await _pumpPage(tester, client);

    expect(client.statusChecks, 0);
    expect(find.text('Runtime not checked'), findsOneWidget);
    final refresh = find.byKey(AppKeys.runtimeRefreshAction);
    expect(tester.widget<OutlinedButton>(refresh).onPressed, isNull);
    expect(client.statusChecks, 0);
  });

  testWidgets('records reuse approval only after dialog confirmation', (
    tester,
  ) async {
    final client = _FakeRuntimeCoreClient(
      _runtimeReport(
        state: 'installed_stopped',
        capabilities: const [
          {
            'kind': 'model_inventory',
            'availability': 'requires_reuse_consent',
            'reason': 'Reuse consent is required.',
          },
        ],
      ),
    );
    addTearDown(client.dispose);
    await _pumpPage(tester, client);

    final action = find.byKey(AppKeys.runtimeConsentAction);
    await tester.ensureVisible(action);
    await tester.tap(action);
    await tester.pumpAndSettle();
    expect(client.consentDecision, isNull);

    await tester.tap(find.text('Allow reuse').last);
    await tester.pumpAndSettle();

    expect(client.consentDecision, RuntimeConsentDecision.approveReuse);
    expect(find.text('Approved for reuse only'), findsOneWidget);
  });

  testWidgets('runtime operation remains cancellable until a terminal event', (
    tester,
  ) async {
    final client = _FakeRuntimeCoreClient(
      _runtimeReport(
        state: 'installed_stopped',
        ownership: 'gix_giz_managed',
        managementConsent: 'management_approved',
        capabilities: const [
          {'kind': 'start', 'availability': 'available', 'reason': null},
        ],
      ),
    );
    addTearDown(client.dispose);
    await _pumpPage(tester, client);

    final start = find.byKey(AppKeys.runtimeStartAction);
    await tester.ensureVisible(start);
    await tester.tap(start);
    await tester.pump();

    expect(client.operationKind, RuntimeOperationKind.start);
    expect(find.byKey(AppKeys.runtimeProgress), findsOneWidget);
    final cancel = find.byKey(AppKeys.runtimeCancelAction);
    await tester.ensureVisible(cancel);
    await tester.tap(cancel);
    await tester.pump();
    expect(client.cancelCount, 1);

    client.emit(
      RuntimeOperationEvent(
        schemaVersion: 1,
        operationId: _operation.operationId,
        correlationId: _operation.correlationId,
        sequence: 1,
        operationKind: RuntimeOperationKind.start,
        kind: RuntimeOperationEventKind.cancelled,
        timestampUnixMs: 1,
        message: 'cancelled',
        report: client.report,
        error: null,
        terminalState: RuntimeOperationTerminalState.cancelled,
      ),
    );
    await tester.pump();

    expect(find.text('Runtime operation cancelled'), findsOneWidget);
  });

  testWidgets('status refresh clears model inventory from the prior report', (
    tester,
  ) async {
    final client = _FakeRuntimeCoreClient(
      _runtimeReport(
        state: 'ready',
        capabilities: const [
          {
            'kind': 'model_inventory',
            'availability': 'available',
            'reason': null,
          },
        ],
      ),
      inventory: _runtimeInventory,
    );
    addTearDown(client.dispose);
    await _pumpPage(tester, client);

    final models = find.byKey(AppKeys.runtimeModelsAction);
    await tester.ensureVisible(models);
    await tester.tap(models);
    await tester.pump();
    expect(find.text('Fixture model'), findsOneWidget);

    final refresh = find.byKey(AppKeys.runtimeRefreshAction);
    await tester.ensureVisible(refresh);
    await tester.tap(refresh);
    await tester.pump();
    await tester.pump();

    expect(find.text('Fixture model'), findsNothing);
  });

  testWidgets('presents typed recovery guidance for a status failure', (
    tester,
  ) async {
    final client = _FakeRuntimeCoreClient(
      _runtimeReport(state: 'ready'),
      statusFailure: const CoreClientFailure(
        code: 'RUNTIME_PREREQUISITE_MISSING',
        category: ErrorCategory.unavailable,
        recoveryAction: RecoveryAction.checkPrerequisites,
      ),
    );
    addTearDown(client.dispose);

    await _pumpPage(tester, client);

    expect(find.text('Runtime check failed'), findsOneWidget);
    expect(
      find.text(
        'Recommended action: Check the runtime prerequisites, then try again.',
      ),
      findsOneWidget,
    );
  });

  testWidgets('presents typed recovery guidance for inventory failure', (
    tester,
  ) async {
    final client = _FakeRuntimeCoreClient(
      _runtimeReport(
        state: 'ready',
        capabilities: const [
          {
            'kind': 'model_inventory',
            'availability': 'available',
            'reason': null,
          },
        ],
      ),
      inventoryFailure: const CoreClientFailure(
        code: 'RUNTIME_INVENTORY_UNAVAILABLE',
        category: ErrorCategory.unavailable,
        recoveryAction: RecoveryAction.contactSupport,
      ),
    );
    addTearDown(client.dispose);
    await _pumpPage(tester, client);

    final models = find.byKey(AppKeys.runtimeModelsAction);
    await tester.ensureVisible(models);
    await tester.tap(models);
    await tester.pump();

    expect(
      find.textContaining('could not list installed models'),
      findsOneWidget,
    );
    expect(
      find.text(
        'Recommended action: Contact support with the diagnostic code.',
      ),
      findsOneWidget,
    );
  });

  testWidgets('identifies a failed lifecycle operation and its recovery', (
    tester,
  ) async {
    final client = _FakeRuntimeCoreClient(
      _runtimeReport(
        state: 'installed_stopped',
        ownership: 'gix_giz_managed',
        managementConsent: 'management_approved',
        capabilities: const [
          {'kind': 'start', 'availability': 'available', 'reason': null},
        ],
      ),
      operationFailure: const CoreClientFailure(
        code: 'RUNTIME_START_DENIED',
        category: ErrorCategory.permissionDenied,
        recoveryAction: RecoveryAction.checkPrerequisites,
      ),
    );
    addTearDown(client.dispose);
    await _pumpPage(tester, client);

    final start = find.byKey(AppKeys.runtimeStartAction);
    await tester.ensureVisible(start);
    await tester.tap(start);
    await tester.pump();

    expect(find.text('Runtime start failed'), findsOneWidget);
    expect(find.textContaining('approved start operation'), findsOneWidget);
    expect(
      find.textContaining('Check the runtime prerequisites'),
      findsOneWidget,
    );
  });

  for (final terminal in const [
    RuntimeOperationTerminalState.cancelled,
    RuntimeOperationTerminalState.failed,
    RuntimeOperationTerminalState.timedOut,
  ]) {
    testWidgets(
      '${terminal.wireValue} terminal event without report clears stale details',
      (tester) async {
        final client = _FakeRuntimeCoreClient(
          _runtimeReport(
            state: 'ready',
            ownership: 'gix_giz_managed',
            managementConsent: 'management_approved',
            capabilities: const [
              {'kind': 'start', 'availability': 'available', 'reason': null},
            ],
          ),
        );
        addTearDown(client.dispose);
        await _pumpPage(tester, client);

        final start = find.byKey(AppKeys.runtimeStartAction);
        await tester.ensureVisible(start);
        await tester.tap(start);
        await tester.pump();
        client.emit(_terminalRuntimeEvent(terminal));
        await tester.pump();

        expect(find.text('Runtime ready'), findsNothing);
        expect(find.text('Managed by GixGiz'), findsNothing);
        expect(find.byKey(AppKeys.runtimeStartAction), findsNothing);
        expect(find.text('Check runtime'), findsOneWidget);
        expect(
          tester
              .widget<OutlinedButton>(find.byKey(AppKeys.runtimeRefreshAction))
              .onPressed,
          isNotNull,
        );
        expect(
          find.text(
            terminal == RuntimeOperationTerminalState.cancelled
                ? 'Runtime operation cancelled'
                : 'Runtime start failed',
          ),
          findsOneWidget,
        );
        expect(client.statusChecks, 1);
      },
    );
  }

  testWidgets('cancelled runtime start preserves the prior report', (
    tester,
  ) async {
    final client = _FakeRuntimeCoreClient(
      _runtimeReport(
        state: 'installed_stopped',
        ownership: 'gix_giz_managed',
        managementConsent: 'management_approved',
        capabilities: const [
          {'kind': 'start', 'availability': 'available', 'reason': null},
        ],
      ),
      operationFailure: const CoreClientFailure(
        code: 'RUNTIME_START_CANCELLED',
        category: ErrorCategory.cancelled,
        recoveryAction: RecoveryAction.noAction,
      ),
    );
    addTearDown(client.dispose);
    await _pumpPage(tester, client);

    final start = find.byKey(AppKeys.runtimeStartAction);
    await tester.ensureVisible(start);
    await tester.tap(start);
    await tester.pump();

    expect(find.text('Runtime operation cancelled'), findsOneWidget);
    expect(find.text('Managed by GixGiz'), findsOneWidget);
    expect(find.text('Runtime start failed'), findsNothing);
  });

  testWidgets('cancelled runtime stream refreshes authoritative status', (
    tester,
  ) async {
    final client = _FakeRuntimeCoreClient(
      _runtimeReport(
        state: 'ready',
        ownership: 'gix_giz_managed',
        managementConsent: 'management_approved',
        capabilities: const [
          {'kind': 'start', 'availability': 'available', 'reason': null},
        ],
      ),
    );
    addTearDown(client.dispose);
    await _pumpPage(tester, client);

    final start = find.byKey(AppKeys.runtimeStartAction);
    await tester.ensureVisible(start);
    await tester.tap(start);
    await tester.pump();
    final refreshed = Completer<RuntimeHealthReport>();
    client.nextStatus = refreshed.future;
    client.emitError(
      const CoreClientFailure(
        code: 'RUNTIME_STREAM_CANCELLED',
        category: ErrorCategory.cancelled,
        recoveryAction: RecoveryAction.noAction,
      ),
    );
    await tester.pump();

    expect(find.text('Runtime operation cancelled'), findsOneWidget);
    expect(find.text('Runtime ready'), findsNothing);
    expect(find.text('Managed by GixGiz'), findsNothing);
    expect(find.text('Runtime start failed'), findsNothing);
    expect(client.cancelCount, 1);
    expect(client.statusChecks, 2);

    refreshed.complete(_runtimeReport(state: 'installed_stopped'));
    await tester.pump();
    await tester.pump();

    expect(find.text('Runtime installed but stopped'), findsOneWidget);
    expect(find.text('Runtime operation cancelled'), findsNothing);
  });

  testWidgets('unterminated runtime stream cancels and refreshes status', (
    tester,
  ) async {
    final client = _FakeRuntimeCoreClient(
      _runtimeReport(
        state: 'ready',
        ownership: 'gix_giz_managed',
        managementConsent: 'management_approved',
        capabilities: const [
          {'kind': 'start', 'availability': 'available', 'reason': null},
        ],
      ),
    );
    addTearDown(client.dispose);
    await _pumpPage(tester, client);

    final start = find.byKey(AppKeys.runtimeStartAction);
    await tester.ensureVisible(start);
    await tester.tap(start);
    await tester.pump();
    final refreshed = Completer<RuntimeHealthReport>();
    client.nextStatus = refreshed.future;
    await client.finishEvents();
    await tester.pump();

    expect(find.text('Runtime start failed'), findsOneWidget);
    expect(find.text('Runtime ready'), findsNothing);
    expect(find.text('Managed by GixGiz'), findsNothing);
    expect(client.cancelCount, 1);
    expect(client.statusChecks, 2);

    refreshed.complete(_runtimeReport(state: 'installed_stopped'));
    await tester.pump();
    await tester.pump();

    expect(find.text('Runtime installed but stopped'), findsOneWidget);
    expect(find.text('Runtime start failed'), findsNothing);
  });

  testWidgets('cancelled cancel request preserves the prior report', (
    tester,
  ) async {
    final client = _FakeRuntimeCoreClient(
      _runtimeReport(
        state: 'installed_stopped',
        ownership: 'gix_giz_managed',
        managementConsent: 'management_approved',
        capabilities: const [
          {'kind': 'start', 'availability': 'available', 'reason': null},
        ],
      ),
      cancelFailure: const CoreClientFailure(
        code: 'RUNTIME_CANCEL_ACKNOWLEDGED',
        category: ErrorCategory.cancelled,
        recoveryAction: RecoveryAction.noAction,
      ),
    );
    addTearDown(client.dispose);
    await _pumpPage(tester, client);

    final start = find.byKey(AppKeys.runtimeStartAction);
    await tester.ensureVisible(start);
    await tester.tap(start);
    await tester.pump();
    final cancel = find.byKey(AppKeys.runtimeCancelAction);
    await tester.ensureVisible(cancel);
    await tester.tap(cancel);
    await tester.pump();
    await tester.pump();

    expect(find.text('Runtime operation cancelled'), findsOneWidget);
    expect(find.text('Managed by GixGiz'), findsOneWidget);
    expect(find.text('Runtime start failed'), findsNothing);
  });

  testWidgets('reuse prompt does not claim an untested runtime is compatible', (
    tester,
  ) async {
    final client = _FakeRuntimeCoreClient(
      _runtimeReport(
        state: 'degraded',
        version: const {
          'reported_version': '0.12.6',
          'normalized_version': '0.12.6',
          'compatibility': 'untested',
        },
        capabilities: const [
          {
            'kind': 'model_inventory',
            'availability': 'requires_reuse_consent',
            'reason': 'Reuse consent is required.',
          },
        ],
      ),
    );
    addTearDown(client.dispose);
    await _pumpPage(tester, client);

    final consent = find.byKey(AppKeys.runtimeConsentAction);
    await tester.ensureVisible(consent);
    await tester.tap(consent);
    await tester.pumpAndSettle();

    expect(
      find.textContaining(
        'when current health and compatibility evidence permits',
      ),
      findsOneWidget,
    );
    expect(find.textContaining('this compatible local runtime'), findsNothing);
  });

  testWidgets('localizes runtime evidence and management-only guidance', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    final client = _FakeRuntimeCoreClient(
      _runtimeReport(
        state: 'degraded',
        capabilities: const [
          {
            'kind': 'start',
            'availability': 'requires_management_consent',
            'reason': 'UNLOCALIZED_CAPABILITY_REASON',
          },
        ],
        reasons: const [
          {'code': 'endpoint_unavailable', 'message': 'UNLOCALIZED_REASON'},
        ],
        warnings: const [
          {'code': 'version_untested', 'message': 'UNLOCALIZED_WARNING'},
        ],
      ),
    );
    addTearDown(client.dispose);
    await _pumpPage(tester, client);

    expect(find.text('Management permission not granted'), findsOneWidget);
    expect(
      find.textContaining('Manage this external installation'),
      findsOneWidget,
    );
    expect(
      find.text('The local runtime endpoint did not respond.'),
      findsOneWidget,
    );
    expect(
      find.text(
        'This runtime version has not been verified with this GixGiz release.',
      ),
      findsOneWidget,
    );
    expect(find.text('UNLOCALIZED_REASON'), findsNothing);
    expect(find.text('UNLOCALIZED_WARNING'), findsNothing);

    final status = tester.getSemantics(find.byKey(AppKeys.runtimeStatus));
    expect(status.label, contains('some health or capability evidence'));
    semantics.dispose();
  });
}

Future<void> _pumpPage(WidgetTester tester, CoreClient client) async {
  await tester.pumpWidget(
    MaterialApp(
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: Scaffold(body: FoundationPage(coreClient: client)),
    ),
  );
  await tester.pump();
  await tester.pump();
}

const _operation = CoreOperation(
  operationId: '00000000-0000-4000-8000-000000000101',
  correlationId: '00000000-0000-4000-8000-000000000102',
);

class _FakeRuntimeCoreClient extends CoreClient {
  _FakeRuntimeCoreClient(
    this.report, {
    this.supportsRuntimeStatus = true,
    this.inventory = _emptyRuntimeInventory,
    this.statusFailure,
    this.operationFailure,
    this.cancelFailure,
    this.inventoryFailure,
  });

  RuntimeHealthReport report;
  final bool supportsRuntimeStatus;
  final RuntimeModelInventory inventory;
  final CoreClientFailure? statusFailure;
  final CoreClientFailure? operationFailure;
  final CoreClientFailure? cancelFailure;
  final CoreClientFailure? inventoryFailure;
  final _events = StreamController<RuntimeOperationEvent>.broadcast();
  Future<RuntimeHealthReport>? nextStatus;
  int statusChecks = 0;
  int cancelCount = 0;
  RuntimeConsentDecision? consentDecision;
  RuntimeOperationKind? operationKind;

  @override
  bool supportsTransportCapability(TransportCapability capability) {
    return switch (capability) {
      TransportCapability.runtimeStatus => supportsRuntimeStatus,
      TransportCapability.runtimeConsent ||
      TransportCapability.runtimeLifecycle ||
      TransportCapability.runtimeModelInventory => true,
      _ => false,
    };
  }

  @override
  Future<CoreConnectionSnapshot> checkConnection() async {
    return const CoreConnectionSnapshot(kind: CoreConnectionKind.ready);
  }

  @override
  Future<RuntimeHealthReport> checkRuntimeStatus() async {
    statusChecks += 1;
    if (statusFailure case final failure?) {
      throw failure;
    }
    final pending = nextStatus;
    if (pending != null) {
      nextStatus = null;
      report = await pending;
    }
    return report;
  }

  @override
  Future<RuntimeHealthReport> decideRuntimeReuse(
    RuntimeConsentDecision decision,
  ) async {
    consentDecision = decision;
    report = _runtimeReport(
      state: 'installed_stopped',
      reuseConsent: decision == RuntimeConsentDecision.approveReuse
          ? 'reuse_approved'
          : 'denied',
    );
    return report;
  }

  @override
  Future<CoreOperation> startRuntimeOperation(RuntimeOperationKind kind) async {
    operationKind = kind;
    if (operationFailure case final failure?) {
      throw failure;
    }
    return _operation;
  }

  @override
  Stream<RuntimeOperationEvent> observeRuntimeOperation(
    CoreOperation operation,
  ) {
    return _events.stream;
  }

  @override
  Future<bool> cancelRuntimeOperation(CoreOperation operation) async {
    cancelCount += 1;
    if (cancelFailure case final failure?) {
      throw failure;
    }
    return true;
  }

  @override
  Future<RuntimeModelInventory> listRuntimeModels() async {
    if (inventoryFailure case final failure?) {
      throw failure;
    }
    return inventory;
  }

  void emit(RuntimeOperationEvent event) => _events.add(event);

  void emitError(CoreClientFailure failure) => _events.addError(failure);

  Future<void> finishEvents() => _events.close();

  Future<void> dispose() =>
      _events.isClosed ? Future<void>.value() : _events.close();
}

RuntimeOperationEvent _terminalRuntimeEvent(
  RuntimeOperationTerminalState terminal,
) {
  final kind = switch (terminal) {
    RuntimeOperationTerminalState.completed =>
      RuntimeOperationEventKind.completed,
    RuntimeOperationTerminalState.cancelled =>
      RuntimeOperationEventKind.cancelled,
    RuntimeOperationTerminalState.failed => RuntimeOperationEventKind.failed,
    RuntimeOperationTerminalState.timedOut =>
      RuntimeOperationEventKind.timedOut,
    RuntimeOperationTerminalState.unknown => RuntimeOperationEventKind.unknown,
  };
  return RuntimeOperationEvent(
    schemaVersion: 1,
    operationId: _operation.operationId,
    correlationId: _operation.correlationId,
    sequence: 1,
    operationKind: RuntimeOperationKind.start,
    kind: kind,
    timestampUnixMs: 1,
    message: null,
    report: null,
    error: null,
    terminalState: terminal,
  );
}

RuntimeHealthReport _runtimeReport({
  required String state,
  String ownership = 'external',
  String reuseConsent = 'not_requested',
  String managementConsent = 'not_requested',
  Map<String, Object?>? version,
  List<Map<String, Object?>> capabilities = const [],
  List<Map<String, Object?>> reasons = const [],
  List<Map<String, Object?>> warnings = const [],
}) {
  return RuntimeHealthReport.fromJson({
    'schema_version': 1,
    'provider_id': 'gixgiz.runtime.ollama.v1',
    'display_name': 'Ollama',
    'state': state,
    'ownership': ownership,
    'reuse_consent': reuseConsent,
    'management_consent': managementConsent,
    'endpoint_safety': 'loopback_verified',
    'version': version,
    'capabilities': capabilities,
    'reasons': reasons,
    'warnings': warnings,
  });
}

const _emptyRuntimeInventory = RuntimeModelInventory(
  collectedAtUnixMs: 1,
  models: [],
  providerId: 'gixgiz.runtime.ollama.v1',
  schemaVersion: 1,
  truncated: false,
);

const _runtimeInventory = RuntimeModelInventory(
  collectedAtUnixMs: 1,
  models: [
    RuntimeModelSummary(
      displayName: 'Fixture model',
      mapping: RuntimeProviderModelMapping(
        catalogueId: null,
        status: RuntimeModelMappingStatus.external,
      ),
      providerModelId: 'fixture:latest',
      sizeBytes: 1024,
    ),
  ],
  providerId: 'gixgiz.runtime.ollama.v1',
  schemaVersion: 1,
  truncated: false,
);
