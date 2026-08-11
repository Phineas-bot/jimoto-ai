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
    expect(find.text('Hardware evidence partially available'), findsOneWidget);
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
    final diagnostics = find.byKey(AppKeys.hardwareScanDiagnostics);
    await tester.ensureVisible(diagnostics);
    await tester.pumpAndSettle();
    await tester.tap(diagnostics);
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

  testWidgets('renders recommended fallback and larger plans with tradeoffs', (
    tester,
  ) async {
    final profile = _machineProfile('complete');
    await _pumpState(
      tester,
      _readyFoundation,
      hardwareScanState: HardwareScanReady(profile: profile),
      recommendationState: CapabilityRecommendationReady(
        report: _plansReport(),
      ),
    );

    expect(find.byKey(AppKeys.capabilityRecommendedPlan), findsOneWidget);
    expect(find.byKey(AppKeys.capabilityFallbackPlan), findsOneWidget);
    expect(find.byKey(AppKeys.capabilityLargerPlan), findsOneWidget);
    expect(find.byKey(AppKeys.capabilityStatus), findsOneWidget);
    expect(find.text('Recommended'), findsOneWidget);
    expect(find.text('Smaller fallback'), findsOneWidget);
    expect(find.text('Larger option'), findsOneWidget);
    expect(find.text('Memory allowance'), findsWidgets);
    expect(find.textContaining('Apache-2.0'), findsWidgets);
    expect(find.textContaining('gixgiz-catalogue-v0.1.0'), findsOneWidget);
  });

  testWidgets(
    'renders unknown and unsupported evidence as an explicit no-plan',
    (tester) async {
      final profile = _machineProfile('partial');
      await _pumpState(
        tester,
        _readyFoundation,
        hardwareScanState: HardwareScanPartial(profile: profile),
        recommendationState: CapabilityRecommendationNoPlan(
          report: _noPlanReport(),
        ),
      );

      expect(find.byKey(AppKeys.capabilityNoPlan), findsOneWidget);
      expect(find.text('No safe plan available'), findsOneWidget);
      expect(find.textContaining('could not be verified'), findsOneWidget);
      expect(find.textContaining('not supported'), findsOneWidget);
    },
  );

  testWidgets('capability preferences and generate action emit intentions', (
    tester,
  ) async {
    WorkloadTier? workload;
    var generated = false;
    await _pumpState(
      tester,
      _readyFoundation,
      hardwareScanState: HardwareScanReady(
        profile: _machineProfile('complete'),
      ),
      onWorkloadChanged: (value) => workload = value,
      onGenerateRecommendation: () => generated = true,
    );

    final coding = find.text('Coding');
    await tester.ensureVisible(coding);
    await tester.pumpAndSettle();
    await tester.tap(coding);
    await tester.pump();
    await tester.ensureVisible(find.byKey(AppKeys.capabilityGenerateAction));
    await tester.tap(find.byKey(AppKeys.capabilityGenerateAction));

    expect(workload, WorkloadTier.coding);
    expect(generated, isTrue);
  });

  testWidgets('capability controls tolerate expanded text', (tester) async {
    await _pumpState(
      tester,
      _readyFoundation,
      hardwareScanState: HardwareScanReady(
        profile: _machineProfile('complete'),
      ),
      textScaler: const TextScaler.linear(2),
    );

    expect(tester.takeException(), isNull);
    expect(find.byKey(AppKeys.capabilityWorkload), findsOneWidget);
  });

  testWidgets('renders every authoritative runtime state distinctly', (
    tester,
  ) async {
    final cases = <String, String>{
      'not_installed': 'Runtime not installed',
      'installed_stopped': 'Runtime installed but stopped',
      'starting': 'Runtime starting',
      'ready': 'Runtime ready',
      'degraded': 'Runtime needs attention',
      'incompatible': 'Runtime version incompatible',
      'updating': 'Runtime updating',
      'failed': 'Runtime check failed',
      'future_state': 'Runtime status unknown',
    };

    for (final entry in cases.entries) {
      await _pumpState(
        tester,
        _readyFoundation,
        runtimeStatusState: RuntimeStatusLoaded(
          report: _runtimeReport(state: entry.key),
        ),
      );
      expect(find.text(entry.value), findsOneWidget);
    }
  });

  testWidgets('external runtime reuse requires explicit confirmation', (
    tester,
  ) async {
    var approved = false;
    await _pumpState(
      tester,
      _readyFoundation,
      runtimeStatusState: RuntimeStatusLoaded(
        report: _runtimeReport(
          state: 'installed_stopped',
          capabilities: const [
            {
              'kind': 'model_inventory',
              'availability': 'requires_reuse_consent',
              'reason': 'Reuse consent is required.',
            },
          ],
        ),
      ),
      onApproveRuntimeReuse: () => approved = true,
    );

    expect(find.text('Installed outside GixGiz'), findsOneWidget);
    expect(find.text('Reuse permission not granted'), findsWidgets);
    expect(find.byKey(AppKeys.runtimeStartAction), findsNothing);
    final consentAction = find.byKey(AppKeys.runtimeConsentAction);
    await tester.ensureVisible(consentAction);
    await tester.tap(consentAction);
    await tester.pumpAndSettle();

    expect(find.text('Allow runtime reuse?'), findsOneWidget);
    expect(find.textContaining('Ownership stays external'), findsOneWidget);
    await tester.tap(find.text('Allow reuse').last);
    await tester.pumpAndSettle();

    expect(approved, isTrue);
  });

  testWidgets('runtime actions emit only core-authorized intentions', (
    tester,
  ) async {
    RuntimeOperationKind? operation;
    var modelsRequested = false;
    await _pumpState(
      tester,
      _readyFoundation,
      runtimeStatusState: RuntimeStatusLoaded(
        report: _runtimeReport(
          state: 'ready',
          ownership: 'gix_giz_managed',
          reuseConsent: 'reuse_approved',
          managementConsent: 'management_approved',
          capabilities: const [
            {'kind': 'start', 'availability': 'available', 'reason': null},
            {'kind': 'stop', 'availability': 'available', 'reason': null},
            {'kind': 'restart', 'availability': 'available', 'reason': null},
            {
              'kind': 'model_inventory',
              'availability': 'available',
              'reason': null,
            },
          ],
        ),
      ),
      onStartRuntimeOperation: (value) => operation = value,
      onToggleRuntimeModels: () => modelsRequested = true,
    );

    final restart = find.byKey(AppKeys.runtimeRestartAction);
    await tester.ensureVisible(restart);
    await tester.tap(restart);
    expect(operation, RuntimeOperationKind.restart);

    final models = find.byKey(AppKeys.runtimeModelsAction);
    await tester.ensureVisible(models);
    await tester.tap(models);
    expect(modelsRequested, isTrue);
  });

  testWidgets('runtime inventory is bounded and marks external models', (
    tester,
  ) async {
    await _pumpState(
      tester,
      _readyFoundation,
      runtimeStatusState: RuntimeStatusLoaded(
        report: _runtimeReport(state: 'ready'),
      ),
      runtimeInventoryState: RuntimeInventoryLoaded(
        inventory: _runtimeInventory(),
      ),
    );

    expect(find.text('Fixture model'), findsOneWidget);
    expect(find.text('External model'), findsOneWidget);
    expect(find.textContaining('More installed models exist'), findsOneWidget);
  });

  testWidgets('runtime failure exposes only a safe diagnostic code', (
    tester,
  ) async {
    await _pumpState(
      tester,
      _readyFoundation,
      runtimeStatusState: RuntimeStatusFailed(
        diagnosticCode: 'runtime.provider_unavailable',
        report: _runtimeReport(state: 'degraded'),
      ),
    );

    expect(find.textContaining('runtime.provider_unavailable'), findsNothing);
    final diagnostics = find.byKey(AppKeys.runtimeDiagnostics);
    await tester.ensureVisible(diagnostics);
    await tester.tap(diagnostics);
    await tester.pumpAndSettle();
    expect(
      find.text('Diagnostic code: runtime.provider_unavailable'),
      findsOneWidget,
    );
  });

  testWidgets('runtime status remains accessible with expanded text', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    try {
      await _pumpState(
        tester,
        _readyFoundation,
        runtimeStatusState: RuntimeStatusLoaded(
          report: _runtimeReport(state: 'ready'),
        ),
        textScaler: const TextScaler.linear(2),
      );

      expect(tester.takeException(), isNull);
      final data = tester
          .getSemantics(find.byKey(AppKeys.runtimeStatus))
          .getSemanticsData();
      expect(data.label, contains('Runtime status: Runtime ready'));
      expect(data.label, contains('Installed outside GixGiz'));
      final details = tester
          .getSemantics(find.byKey(AppKeys.runtimeDetails))
          .getSemanticsData();
      expect(details.label, contains('management permission'));
      expect(details.label, contains('Reuse permission not granted'));
    } finally {
      semantics.dispose();
    }
  });
}

const _readyFoundation = FoundationReady(
  snapshot: CoreConnectionSnapshot(kind: CoreConnectionKind.ready),
);

Future<void> _pumpState(
  WidgetTester tester,
  FoundationState state, {
  HardwareScanState hardwareScanState = const HardwareScanIdle(),
  CapabilityRecommendationState recommendationState =
      const CapabilityRecommendationIdle(),
  RuntimeStatusState runtimeStatusState = const RuntimeStatusIdle(),
  RuntimeInventoryState runtimeInventoryState = const RuntimeInventoryIdle(),
  ValueChanged<WorkloadTier>? onWorkloadChanged,
  VoidCallback? onGenerateRecommendation,
  VoidCallback? onApproveRuntimeReuse,
  ValueChanged<RuntimeOperationKind>? onStartRuntimeOperation,
  VoidCallback? onToggleRuntimeModels,
  TextScaler textScaler = TextScaler.noScaling,
}) async {
  await tester.pumpWidget(
    MediaQuery(
      data: MediaQueryData(textScaler: textScaler),
      child: MaterialApp(
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: Scaffold(
          body: FoundationScreen(
            state: state,
            hardwareScanState: hardwareScanState,
            recommendationState: recommendationState,
            runtimeStatusState: runtimeStatusState,
            runtimeInventoryState: runtimeInventoryState,
            onRetry: () {},
            onStartHardwareScan: () {},
            onCancelHardwareScan: () {},
            onWorkloadChanged: onWorkloadChanged,
            onGenerateRecommendation: onGenerateRecommendation,
            onApproveRuntimeReuse: onApproveRuntimeReuse,
            onStartRuntimeOperation: onStartRuntimeOperation,
            onToggleRuntimeModels: onToggleRuntimeModels,
          ),
        ),
      ),
    ),
  );
  await tester.pump();
}

RuntimeHealthReport _runtimeReport({
  required String state,
  String ownership = 'external',
  String reuseConsent = 'not_requested',
  String managementConsent = 'not_requested',
  List<Map<String, Object?>> capabilities = const [],
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
    'version': {
      'reported_version': '0.11.10',
      'normalized_version': '0.11.10',
      'compatibility': 'compatible',
    },
    'capabilities': capabilities,
    'reasons': <Object?>[],
    'warnings': <Object?>[],
  });
}

RuntimeModelInventory _runtimeInventory() {
  return RuntimeModelInventory.fromJson({
    'schema_version': 1,
    'provider_id': 'gixgiz.runtime.ollama.v1',
    'models': [
      {
        'provider_model_id': 'fixture:latest',
        'display_name': 'Fixture model',
        'size_bytes': 1073741824,
        'mapping': {'status': 'external', 'catalogue_id': null},
      },
    ],
    'truncated': true,
    'collected_at_unix_ms': 1,
  });
}

CapabilityReport _plansReport() {
  return CapabilityReport.fromJson({
    ..._reportBase,
    'status': 'plans_available',
    'recommended_plan': _plan('recommended', 'Qwen 2.5 1.5B Instruct'),
    'fallback_plan': _plan('fallback', 'Qwen 2.5 0.5B Instruct'),
    'optional_larger_plan': _plan('optional_larger', 'Qwen 2.5 7B Instruct'),
    'no_plan': null,
    'confidence': 'high',
    'reasons': [
      {
        'code': 'balanced_choice',
        'message': 'Balances capability and headroom.',
      },
    ],
    'warnings': <Object?>[],
  });
}

CapabilityReport _noPlanReport() {
  final reasons = [
    {
      'code': 'critical_evidence_unknown',
      'message': 'Available memory could not be verified.',
    },
    {
      'code': 'unsupported_architecture',
      'message': 'This machine type is not supported.',
    },
  ];
  return CapabilityReport.fromJson({
    ..._reportBase,
    'status': 'no_plan',
    'recommended_plan': null,
    'fallback_plan': null,
    'optional_larger_plan': null,
    'no_plan': {
      'confidence': 'low',
      'reasons': reasons,
      'warnings': [
        {
          'code': 'no_safe_plan',
          'message': 'More reliable evidence is needed.',
        },
      ],
    },
    'confidence': 'low',
    'reasons': reasons,
    'warnings': <Object?>[],
  });
}

const _reportBase = <String, Object?>{
  'schema_version': 1,
  'catalogue_version': 'gixgiz-catalogue-v0.1.0',
  'rule_set_version': 'gixgiz-capability-rules-v0.1.0',
  'machine_profile_schema_version': 1,
  'generated_from_scan_unix_ms': 1,
  'preferences': {
    'workload': 'general_text',
    'priority': 'balanced',
    'include_optional_larger': true,
  },
};

Map<String, Object?> _plan(String role, String name) {
  return {
    'catalogue_version': 'gixgiz-catalogue-v0.1.0',
    'rule_set_version': 'gixgiz-capability-rules-v0.1.0',
    'role': role,
    'compatibility': 'compatible',
    'model': {
      'catalogue_id': 'gixgiz.model.fixture',
      'display_name': name,
      'family': 'Qwen 2.5',
      'licence_spdx': 'Apache-2.0',
      'provenance_url': 'https://huggingface.co/Qwen/fixture',
      'size_class': 'standard',
      'workload_tiers': ['general_text'],
    },
    'runtime': {
      'catalogue_id': 'gixgiz.runtime.local-text',
      'display_name': 'GixGiz local text runtime',
      'supports_cpu_only': true,
      'supported_architectures': ['x86_64'],
      'optional_accelerations': ['direct_ml'],
    },
    'resources': {
      'memory': {
        'required_bytes': 4294967296,
        'safety_margin_bytes': 3221225472,
        'observed_total_bytes': 17179869184,
        'observed_available_bytes': 10737418240,
      },
      'storage': {
        'required_bytes': 2147483648,
        'safety_margin_bytes': 2147483648,
        'observed_free_bytes': 32212254720,
      },
      'planned_context_tokens': 8192,
      'cpu_only': true,
      'gpu_memory_bytes': null,
      'acceleration': null,
    },
    'confidence': 'high',
    'reasons': [
      {'code': 'workload_match', 'message': 'Matches the selected use.'},
    ],
    'warnings': <Object?>[],
  };
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
          'dedicated_memory_bytes': {'value': null, 'metadata': unreliable},
          'shared_memory_bytes': {'value': null, 'metadata': unreliable},
        },
      ],
      'metadata': available,
    },
    'acceleration': [
      {'kind': 'direct_ml', 'supported': null, 'metadata': unreliable},
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
