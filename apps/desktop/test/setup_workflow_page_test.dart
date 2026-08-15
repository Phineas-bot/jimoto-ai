import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gixgiz_desktop/app/app_keys.dart';
import 'package:gixgiz_desktop/core/core_client.dart';
import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';
import 'package:gixgiz_desktop/features/foundation/foundation_page.dart';
import 'package:gixgiz_desktop/l10n/app_localizations.dart';

import 'support/setup_fixture.dart';

void main() {
  testWidgets('disposing the page detaches without cancelling the setup job', (
    tester,
  ) async {
    final client = _SetupCoreClient(
      setupJobFixture(state: 'active', stage: 'acquiring'),
    );

    await _pumpPage(tester, client);
    expect(find.byKey(AppKeys.setupPanel), findsOneWidget);
    expect(client.streamListenCount, 1);

    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump();

    expect(client.streamCancelCount, 1);
    expect(client.cancelRequestCount, 0);
    await client.close();
  });

  testWidgets('only the confirmed cancel action sends cancellation', (
    tester,
  ) async {
    final client = _SetupCoreClient(
      setupJobFixture(state: 'active', stage: 'acquiring'),
    );
    await _pumpPage(tester, client);

    final cancel = find.byKey(AppKeys.setupCancelAction);
    await tester.ensureVisible(cancel);
    await tester.tap(cancel);
    await tester.pump(const Duration(milliseconds: 300));
    expect(client.cancelRequestCount, 0);

    await tester.tap(find.widgetWithText(FilledButton, 'Cancel setup').last);
    await tester.pump(const Duration(milliseconds: 500));
    await tester.pump();

    expect(find.text('Cancel model setup?'), findsNothing);
    expect(client.streamCancelCount, 1);
    expect(client.cancelRequestCount, 1);
    expect(find.text('Cancellation requested'), findsOneWidget);
    final disabledCancel = tester.widget<OutlinedButton>(
      find.byKey(AppKeys.setupCancelAction),
    );
    expect(disabledCancel.onPressed, isNull);
    await tester.pumpWidget(const SizedBox.shrink());
    await client.close();
  });

  testWidgets('approved and attention jobs require confirmed cancellation', (
    tester,
  ) async {
    const cases = <(String, String)>[
      ('approved', 'approved'),
      ('attention_required', 'attention_required'),
    ];

    for (final (state, stage) in cases) {
      final client = _SetupCoreClient(
        setupJobFixture(state: state, stage: stage),
      );
      await _pumpPage(tester, client);

      final cancel = find.byKey(AppKeys.setupCancelAction);
      await tester.ensureVisible(cancel);
      await tester.tap(cancel);
      await tester.pump();
      expect(client.cancelRequestCount, 0);

      await tester.tap(find.widgetWithText(FilledButton, 'Cancel setup').last);
      await tester.pumpAndSettle();
      expect(client.cancelRequestCount, 1);

      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pump();
      await client.close();
    }
  });

  testWidgets('ready is rendered only after an authoritative persisted event', (
    tester,
  ) async {
    final client = _SetupCoreClient(
      setupJobFixture(state: 'active', stage: 'acquiring'),
    );
    await _pumpPage(tester, client);
    expect(find.text('Local model ready'), findsNothing);

    final ready = setupJobFixture(
      state: 'ready',
      stage: 'ready',
      updatedAtUnixMs: 8,
    );
    client.addEvent(
      setupEventFixture(
        sequence: 1,
        job: ready,
        kind: 'ready',
        terminalState: 'ready',
      ),
    );
    await tester.pump();

    expect(find.text('Local model ready'), findsOneWidget);
    expect(client.cancelRequestCount, 0);
    await tester.pumpWidget(const SizedBox.shrink());
    await client.close();
  });

  testWidgets('recovery resumes after the latest retained event cursor', (
    tester,
  ) async {
    final client = _SetupCoreClient(
      setupJobFixture(
        state: 'active',
        stage: 'acquiring',
        latestEventSequence: 41,
      ),
    );

    await _pumpPage(tester, client);

    expect(client.observedAfterSequence, 41);
    client.addEvent(
      setupEventFixture(
        sequence: 42,
        job: setupJobFixture(
          state: 'active',
          stage: 'verifying_model',
          latestEventSequence: 42,
          updatedAtUnixMs: 8,
        ),
      ),
    );
    await tester.pump();
    expect(find.text('Verifying model'), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
    await client.close();
  });

  testWidgets('transport recovery remains actionable with safe diagnostics', (
    tester,
  ) async {
    final client = _SetupCoreClient(
      setupJobFixture(),
      recoverFailure: const CoreClientFailure(
        code: 'SETUP_RECOVERY_UNAVAILABLE',
        category: ErrorCategory.unavailable,
        recoveryAction: RecoveryAction.contactSupport,
        recoveryMessage: 'Include SETUP_RECOVERY_UNAVAILABLE with diagnostics.',
      ),
    );

    await _pumpPage(tester, client);

    expect(find.text('Model setup failed'), findsOneWidget);
    expect(
      find.text(
        'Recommended action: Contact support with the diagnostic code.',
      ),
      findsOneWidget,
    );
    await tester.ensureVisible(find.byKey(AppKeys.setupDiagnostics));
    await tester.tap(find.byKey(AppKeys.setupDiagnostics));
    await tester.pumpAndSettle();
    expect(
      find.text('Include SETUP_RECOVERY_UNAVAILABLE with diagnostics.'),
      findsOneWidget,
    );

    await tester.pumpWidget(const SizedBox.shrink());
    await client.close();
  });

  testWidgets('failed and cancelled jobs allow reviewing a new plan', (
    tester,
  ) async {
    const terminalCases = <(String, String)>[
      ('failed', 'failed'),
      ('cancelled', 'cancelled'),
    ];

    for (final (state, stage) in terminalCases) {
      final client = _SetupCoreClient(
        setupJobFixture(state: state, stage: stage),
      );
      await _pumpPage(tester, client);

      await tester.ensureVisible(find.byKey(AppKeys.hardwareScanPrimaryAction));
      await tester.tap(find.byKey(AppKeys.hardwareScanPrimaryAction));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.byKey(AppKeys.capabilityGenerateAction));
      await tester.tap(find.byKey(AppKeys.capabilityGenerateAction));
      await tester.pumpAndSettle();

      final review = find.byKey(AppKeys.setupReviewAction);
      expect(review, findsOneWidget);
      await tester.ensureVisible(review);
      await tester.tap(review);
      await tester.pumpAndSettle();
      expect(client.createPlanCount, 1);

      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pump();
      await client.close();
    }
  });

  testWidgets(
    'failed contact-support job offers a new plan without same-plan retry',
    (tester) async {
      final client = _SetupCoreClient(
        setupJobFixture(
          state: 'failed',
          stage: 'failed',
          recoveryAction: SetupRecoveryAction.contactSupport,
        ),
      );
      await _pumpPage(tester, client);

      expect(find.byKey(AppKeys.setupRetryAction), findsNothing);

      await tester.ensureVisible(find.byKey(AppKeys.hardwareScanPrimaryAction));
      await tester.tap(find.byKey(AppKeys.hardwareScanPrimaryAction));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.byKey(AppKeys.capabilityGenerateAction));
      await tester.tap(find.byKey(AppKeys.capabilityGenerateAction));
      await tester.pumpAndSettle();

      final review = find.byKey(AppKeys.setupReviewAction);
      expect(review, findsOneWidget);
      await tester.ensureVisible(review);
      await tester.tap(review);
      await tester.pumpAndSettle();
      expect(client.createPlanCount, 1);

      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pump();
      await client.close();
    },
  );
}

Future<void> _pumpPage(WidgetTester tester, CoreClient client) async {
  await tester.pumpWidget(
    MaterialApp(
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: FoundationPage(coreClient: client),
    ),
  );
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 20));
}

class _SetupCoreClient extends CoreClient {
  _SetupCoreClient(this.job, {this.recoverFailure}) {
    events = StreamController<SetupJobEvent>.broadcast(
      onListen: () => streamListenCount += 1,
      onCancel: () {
        streamCancelCount += 1;
      },
      sync: true,
    );
  }

  SetupJobSnapshot job;
  final CoreClientFailure? recoverFailure;
  late final StreamController<SetupJobEvent> events;
  int streamListenCount = 0;
  int streamCancelCount = 0;
  int cancelRequestCount = 0;
  int createPlanCount = 0;
  int? observedAfterSequence;

  @override
  bool supportsTransportCapability(TransportCapability capability) {
    return capability == TransportCapability.setupWorkflow ||
        capability == TransportCapability.hardwareScan ||
        capability == TransportCapability.capabilityRecommendation;
  }

  @override
  Future<CoreConnectionSnapshot> checkConnection() async {
    return const CoreConnectionSnapshot(kind: CoreConnectionKind.ready);
  }

  @override
  Future<SetupJobSnapshot?> recoverSetupJob() async {
    if (recoverFailure case final failure?) {
      throw failure;
    }
    return job;
  }

  @override
  Future<CoreOperation> startHardwareScan() async {
    return const CoreOperation(
      operationId: '00000000-0000-4000-8000-000000000020',
      correlationId: '00000000-0000-4000-8000-000000000021',
    );
  }

  @override
  Stream<HardwareScanEvent> observeHardwareScan(
    CoreOperation operation,
  ) async* {
    yield HardwareScanEvent(
      schemaVersion: 1,
      operationId: operation.operationId,
      correlationId: operation.correlationId,
      sequence: 1,
      kind: HardwareScanEventKind.completed,
      timestampUnixMs: 1,
      message: null,
      profile: _machineProfile(),
      error: null,
      terminalState: HardwareScanTerminalState.completed,
    );
  }

  @override
  Future<bool> cancelHardwareScan(CoreOperation operation) async => true;

  @override
  Future<CapabilityReport> recommendCapability(
    MachineProfile profile,
    UserPreferenceProfile preferences,
  ) async {
    return _capabilityReport(preferences);
  }

  @override
  Future<SetupJobSnapshot> createSetupPlan(
    RecommendationPlan recommendation, {
    SetupDestinationCategory destination =
        SetupDestinationCategory.providerManaged,
  }) async {
    createPlanCount += 1;
    job = setupJobFixture();
    return job;
  }

  @override
  Stream<SetupJobEvent> observeSetupJob(
    SetupJobId jobId, {
    required int afterSequence,
  }) {
    observedAfterSequence = afterSequence;
    return events.stream.where((event) => event.sequence > afterSequence);
  }

  @override
  Future<SetupJobSnapshot> setupJobStatus(SetupJobId jobId) async => job;

  @override
  Future<SetupJobSnapshot> cancelSetupJob(SetupJobId jobId) async {
    cancelRequestCount += 1;
    final active = job.state == SetupJobState.active;
    job = setupJobFixture(
      state: active ? 'active' : 'cancelled',
      stage: active ? 'acquiring' : 'cancelled',
      cancellationRequested: active,
      updatedAtUnixMs: job.updatedAtUnixMs + 1,
    );
    return job;
  }

  void addEvent(SetupJobEvent event) {
    job = event.job;
    events.add(event);
  }

  Future<void> close() => events.close();
}

MachineProfile _machineProfile() {
  const metadata = <String, Object?>{
    'source': 'windows_cim',
    'availability': 'available',
    'confidence': 'high',
    'reason_code': null,
    'reason': null,
  };
  Map<String, Object?> evidence(Object value) => {
    'value': value,
    'metadata': metadata,
  };
  return MachineProfile.fromJson({
    'schema_version': 1,
    'scan_id': '00000000-0000-4000-8000-000000000022',
    'correlation_id': '00000000-0000-4000-8000-000000000023',
    'scanned_at_unix_ms': 1,
    'completeness': 'complete',
    'operating_system': {
      'name': evidence('Windows 11'),
      'version': evidence('10.0'),
      'build': evidence('26100'),
      'architecture': evidence('x86_64'),
    },
    'cpu': {
      'name': evidence('Fixture CPU'),
      'vendor': evidence('Fixture vendor'),
      'physical_core_count': evidence(4),
      'logical_core_count': evidence(8),
    },
    'physical_memory': {
      'total_bytes': evidence(16 * 1024 * 1024 * 1024),
      'available_bytes': evidence(10 * 1024 * 1024 * 1024),
    },
    'gpus': {'devices': <Object?>[], 'metadata': metadata},
    'acceleration': <Object?>[],
    'storage': {
      'location': 'application_data',
      'capacity_bytes': evidence(200 * 1024 * 1024 * 1024),
      'free_bytes': evidence(30 * 1024 * 1024 * 1024),
      'filesystem': evidence('NTFS'),
      'media_kind': evidence('fixed'),
    },
  });
}

CapabilityReport _capabilityReport(UserPreferenceProfile preferences) {
  return CapabilityReport.fromJson({
    'schema_version': 1,
    'catalogue_version': 'gixgiz-catalogue-v0.1.0',
    'rule_set_version': 'gixgiz-capability-rules-v0.1.0',
    'machine_profile_schema_version': 1,
    'generated_from_scan_unix_ms': 1,
    'preferences': preferences.toJson(),
    'status': 'plans_available',
    'recommended_plan': setupRecommendationFixture().toJson(),
    'fallback_plan': null,
    'optional_larger_plan': null,
    'no_plan': null,
    'confidence': 'high',
    'reasons': <Object?>[],
    'warnings': <Object?>[],
  });
}
