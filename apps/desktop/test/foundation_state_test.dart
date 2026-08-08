import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gixgiz_desktop/app/app_keys.dart';
import 'package:gixgiz_desktop/core/core_client.dart';
import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';
import 'package:gixgiz_desktop/features/foundation/foundation_screen.dart';
import 'package:gixgiz_desktop/features/foundation/foundation_state.dart';
import 'package:gixgiz_desktop/l10n/app_localizations.dart';

void main() {
  testWidgets('renders loading state', (tester) async {
    await _pumpState(tester, const FoundationLoading());

    expect(find.text('Checking foundation'), findsOneWidget);
    expect(find.byKey(AppKeys.foundationProgress), findsOneWidget);
    expect(find.byKey(AppKeys.primaryAction), findsNothing);
  });

  testWidgets('renders real core version protocol and readiness', (
    tester,
  ) async {
    await _pumpState(
      tester,
      const FoundationReady(
        snapshot: CoreConnectionSnapshot(
          kind: CoreConnectionKind.ready,
          applicationName: 'GixGiz',
          coreVersion: '0.1.0',
          protocolVersion: 1,
          readiness: ReadinessStatus.ready,
          readinessSummary: 'All mandatory platform services are healthy.',
        ),
      ),
    );

    expect(find.text('Platform core ready'), findsOneWidget);
    expect(find.text('0.1.0'), findsOneWidget);
    expect(find.text('1'), findsOneWidget);
    expect(find.text('ready'), findsOneWidget);
    expect(find.byKey(AppKeys.coreDetails), findsOneWidget);
    expect(find.byKey(AppKeys.primaryAction), findsNothing);
  });

  testWidgets('renders degraded state with recovery action', (tester) async {
    await _pumpState(
      tester,
      const FoundationDegraded(
        snapshot: CoreConnectionSnapshot(
          kind: CoreConnectionKind.degraded,
          coreVersion: '0.1.0',
          protocolVersion: 1,
          readiness: ReadinessStatus.degraded,
        ),
      ),
    );

    expect(find.text('Core connection degraded'), findsOneWidget);
    expect(find.byKey(AppKeys.primaryAction), findsOneWidget);
  });

  testWidgets('renders missing core and startup timeout distinctly', (
    tester,
  ) async {
    await _pumpState(
      tester,
      const FoundationUnavailable(
        issue: CoreConnectionIssue.missingCore,
        diagnosticCode: 'CORE_EXECUTABLE_MISSING',
      ),
    );
    expect(find.text('Core component missing'), findsOneWidget);

    await _pumpState(
      tester,
      const FoundationUnavailable(
        issue: CoreConnectionIssue.startupTimedOut,
        diagnosticCode: 'CORE_STARTUP_TIMEOUT',
      ),
    );
    expect(find.text('Core startup timed out'), findsOneWidget);
  });

  testWidgets('renders protocol and authentication failures distinctly', (
    tester,
  ) async {
    await _pumpState(
      tester,
      const FoundationFailed(
        issue: CoreConnectionIssue.protocolMismatch,
        diagnosticCode: 'CORE_PROTOCOL_INCOMPATIBLE',
      ),
    );
    expect(find.text('Core update required'), findsOneWidget);

    await _pumpState(
      tester,
      const FoundationFailed(
        issue: CoreConnectionIssue.authenticationFailed,
        diagnosticCode: 'CORE_AUTHENTICATION_FAILED',
      ),
    );
    expect(find.text('Core authentication failed'), findsOneWidget);
  });

  testWidgets('renders failed state with expandable safe diagnostic code', (
    tester,
  ) async {
    await _pumpState(
      tester,
      const FoundationFailed(
        issue: CoreConnectionIssue.internalFailure,
        diagnosticCode: 'CORE_TEST_FAILURE',
      ),
    );

    expect(find.text('Foundation check failed'), findsOneWidget);
    expect(find.textContaining('CORE_TEST_FAILURE'), findsNothing);

    await tester.tap(find.byKey(AppKeys.diagnostics));
    await tester.pumpAndSettle();

    expect(find.text('Diagnostic code: CORE_TEST_FAILURE'), findsOneWidget);
  });

  testWidgets('renders cancelled state separately from failure', (
    tester,
  ) async {
    await _pumpState(tester, const FoundationCancelled());

    expect(find.text('Foundation check cancelled'), findsOneWidget);
    expect(find.textContaining('No system changes were made'), findsOneWidget);
    expect(find.byKey(AppKeys.primaryAction), findsOneWidget);
  });

  testWidgets('renders hardware ready and partial profiles distinctly', (
    tester,
  ) async {
    final profile = _machineProfile('complete');
    await _pumpState(
      tester,
      _readyFoundation,
      hardwareScanState: HardwareScanReady(profile: profile),
    );
    expect(find.text('Hardware evidence ready'), findsOneWidget);
    expect(find.textContaining('Example CPU'), findsOneWidget);

    await _pumpState(
      tester,
      _readyFoundation,
      hardwareScanState: HardwareScanPartial(
        profile: _machineProfile('partial'),
      ),
    );
    expect(
      find.text('Hardware evidence partially available'),
      findsOneWidget,
    );
    expect(find.textContaining('Source:'), findsWidgets);
  });

  testWidgets('renders hardware failed and cancelled states accessibly', (
    tester,
  ) async {
    await _pumpState(
      tester,
      _readyFoundation,
      hardwareScanState: const HardwareScanFailed(
        diagnosticCode: 'hardware.provider_unavailable',
      ),
    );
    expect(find.text('Hardware scan failed'), findsOneWidget);
    expect(find.byKey(AppKeys.hardwareScanStatus), findsOneWidget);
    await tester.tap(find.byKey(AppKeys.hardwareScanDiagnostics));
    await tester.pumpAndSettle();
    expect(
      find.text('Diagnostic code: hardware.provider_unavailable'),
      findsOneWidget,
    );

    await _pumpState(
      tester,
      _readyFoundation,
      hardwareScanState: const HardwareScanCancelled(),
    );
    expect(find.text('Hardware scan cancelled'), findsOneWidget);
    expect(find.byKey(AppKeys.hardwareScanPrimaryAction), findsOneWidget);
  });
}

const _readyFoundation = FoundationReady(
  snapshot: CoreConnectionSnapshot(kind: CoreConnectionKind.ready),
);

Future<void> _pumpState(
  WidgetTester tester,
  FoundationState state, {
  HardwareScanState hardwareScanState = const HardwareScanIdle(),
}) async {
  await tester.pumpWidget(
    MaterialApp(
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: Scaffold(
        body: FoundationScreen(
          state: state,
          hardwareScanState: hardwareScanState,
          onRetry: () {},
          onStartHardwareScan: () {},
          onCancelHardwareScan: () {},
        ),
      ),
    ),
  );
  await tester.pump();
}

MachineProfile _machineProfile(String completeness) {
  const available = <String, Object?>{
    'source': 'windows_cim',
    'availability': 'available',
    'confidence': 'high',
    'reason_code': null,
    'reason': null,
  };
  const unreliable = <String, Object?>{
    'source': 'windows_cim',
    'availability': 'not_reliable',
    'confidence': 'unknown',
    'reason_code': 'source_unreliable',
    'reason': 'The source cannot report this value reliably.',
  };
  Map<String, Object?> text(String value) => {
    'value': value,
    'metadata': available,
  };
  Map<String, Object?> number(int value) => {
    'value': value,
    'metadata': available,
  };

  return MachineProfile.fromJson({
    'schema_version': 1,
    'scan_id': '00000000-0000-4000-8000-000000000001',
    'correlation_id': '00000000-0000-4000-8000-000000000002',
    'scanned_at_unix_ms': 1,
    'completeness': completeness,
    'operating_system': {
      'name': text('Windows 11'),
      'version': text('10.0'),
      'build': text('26100'),
      'architecture': {'value': 'x86_64', 'metadata': available},
    },
    'cpu': {
      'name': text('Example CPU'),
      'vendor': text('Example vendor'),
      'physical_core_count': number(4),
      'logical_core_count': number(8),
    },
    'physical_memory': {
      'total_bytes': number(17179869184),
      'available_bytes': number(8589934592),
    },
    'gpus': {
      'devices': [
        {
          'name': text('Example GPU'),
          'vendor': text('Example vendor'),
          'dedicated_memory_bytes': {
            'value': null,
            'metadata': unreliable,
          },
          'shared_memory_bytes': {
            'value': null,
            'metadata': unreliable,
          },
        },
      ],
      'metadata': available,
    },
    'acceleration': [
      {
        'kind': 'direct_ml',
        'supported': null,
        'metadata': unreliable,
      },
    ],
    'storage': {
      'location': 'application_data',
      'capacity_bytes': number(1000000000),
      'free_bytes': number(500000000),
      'filesystem': text('NTFS'),
      'media_kind': {'value': 'fixed', 'metadata': available},
    },
  });
}
