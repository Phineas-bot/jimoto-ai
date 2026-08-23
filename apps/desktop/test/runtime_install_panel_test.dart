import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';
import 'package:gixgiz_desktop/app/app_keys.dart';
import 'package:gixgiz_desktop/features/foundation/foundation_state.dart';
import 'package:gixgiz_desktop/features/foundation/runtime_install_panel.dart';
import 'package:gixgiz_desktop/features/foundation/runtime_panel.dart';
import 'package:gixgiz_desktop/l10n/app_localizations.dart';

Widget _host(Widget child) {
  return MaterialApp(
    localizationsDelegates: const [
      AppLocalizations.delegate,
      GlobalMaterialLocalizations.delegate,
      GlobalWidgetsLocalizations.delegate,
      GlobalCupertinoLocalizations.delegate,
    ],
    supportedLocales: AppLocalizations.supportedLocales,
    home: Scaffold(body: SingleChildScrollView(child: child)),
  );
}

RuntimeInstallPlan _plan({bool requiresAdministrator = false}) {
  return RuntimeInstallPlan(
    schemaVersion: 1,
    jobId: '11111111-1111-4111-8111-111111111111',
    revision: 1,
    providerId: 'ollama',
    runtimeDisplayName: 'Local AI runtime',
    components: [
      RuntimeInstallComponent(
        displayName: 'Local AI runtime',
        version: '0.12.6',
        sourceOrigin: 'https://github.com/ollama/ollama/releases',
        artifactName: 'OllamaSetup.exe',
        expectedSizeBytes: 750 * 1024 * 1024,
        integrityEvidence: RuntimeInstallIntegrityEvidence.digestAndPublisher,
        expectedPublisher: 'Ollama Inc.',
      ),
    ],
    destination: RuntimeInstallDestinationCategory.perUserApplicationDirectory,
    expectedDownloadBytes: 750 * 1024 * 1024,
    requiresAdministrator: requiresAdministrator,
    ownershipAfterSuccess: RuntimeOwnership.gixGizManaged,
    authorizedEffects: const [
      RuntimeInstallEffectKind.stagedInstallerArtifact,
      RuntimeInstallEffectKind.perUserApplicationFiles,
    ],
    reasons: const ['No local AI runtime was found on this PC.'],
    warnings: const ['Installing writes files outside the GixGiz data folder.'],
  );
}

RuntimeInstallJobSnapshot _job(RuntimeInstallState state) {
  return RuntimeInstallJobSnapshot(
    schemaVersion: 1,
    jobId: '11111111-1111-4111-8111-111111111111',
    state: state,
    stage: RuntimeInstallStage.awaitingApproval,
    plan: _plan(),
    approval: null,
    progress: null,
    verification: null,
    effects: RuntimeInstallEffectReport(effects: const []),
    attention: null,
    failure: null,
    recovery: null,
    ownership: RuntimeOwnership.unknown,
    retryCount: 0,
    updatedAtUnixMs: 1,
  );
}

void main() {
  testWidgets('the plan discloses source, publisher and material effects', (
    tester,
  ) async {
    await tester.pumpWidget(
      _host(RuntimeInstallPanel(plan: _plan(), job: _job(RuntimeInstallState.awaitingApproval))),
    );

    expect(find.textContaining('github.com/ollama/ollama'), findsOneWidget);
    expect(find.text('Ollama Inc.'), findsOneWidget);
    expect(find.text('0.12.6'), findsOneWidget);
    expect(
      find.textContaining('outside the GixGiz data folder'),
      findsOneWidget,
    );
  });

  testWidgets('a per-user plan states that no administrator is needed', (
    tester,
  ) async {
    await tester.pumpWidget(_host(RuntimeInstallPanel(plan: _plan())));

    expect(find.text('No, this installs just for you'), findsOneWidget);
    expect(
      find.text('Yes, Windows will ask for permission'),
      findsNothing,
    );
  });

  testWidgets('an existing runtime is reported as reused, not replaced', (
    tester,
  ) async {
    await tester.pumpWidget(
      _host(
        const RuntimeInstallPanel(
          attention: RuntimeInstallAttentionReason.externalRuntimePresent,
        ),
      ),
    );

    expect(
      find.textContaining('already installed'),
      findsOneWidget,
    );
  });

  testWidgets('approval and denial are both offered before any work', (
    tester,
  ) async {
    var approved = false;
    var denied = false;
    await tester.pumpWidget(
      _host(
        RuntimeInstallPanel(
          plan: _plan(),
          job: _job(RuntimeInstallState.awaitingApproval),
          onApprove: () => approved = true,
          onDeny: () => denied = true,
        ),
      ),
    );

    await tester.tap(find.text('Approve and continue'));
    await tester.tap(find.text('Not now'));
    expect(approved, isTrue);
    expect(denied, isTrue);
  });

  testWidgets('a running installation never claims to be ready', (
    tester,
  ) async {
    await tester.pumpWidget(
      _host(
        RuntimeInstallPanel(
          plan: _plan(),
          job: _job(RuntimeInstallState.running),
        ),
      ),
    );

    expect(find.textContaining('Setting up'), findsOneWidget);
    expect(find.textContaining('installed and verified'), findsNothing);
    // Only the core may report readiness, so no start/approve action remains.
    expect(find.text('Approve and continue'), findsNothing);
  });

  testWidgets('readiness is shown only for a verified installation', (
    tester,
  ) async {
    await tester.pumpWidget(
      _host(
        RuntimeInstallPanel(plan: _plan(), job: _job(RuntimeInstallState.ready)),
      ),
    );

    expect(find.textContaining('installed and verified'), findsOneWidget);
  });

  testWidgets('a cancelled plan states that nothing was changed', (
    tester,
  ) async {
    await tester.pumpWidget(
      _host(
        RuntimeInstallPanel(
          plan: _plan(),
          job: _job(RuntimeInstallState.cancelled),
        ),
      ),
    );

    expect(find.textContaining('Nothing on this PC was changed'), findsOneWidget);
  });

  testWidgets('actions are disabled while work is in flight', (tester) async {
    var approved = false;
    await tester.pumpWidget(
      _host(
        RuntimeInstallPanel(
          plan: _plan(),
          job: _job(RuntimeInstallState.awaitingApproval),
          busy: true,
          onApprove: () => approved = true,
        ),
      ),
    );

    await tester.tap(find.text('Approve and continue'));
    expect(approved, isFalse);
  });

  group('untested runtime version', () {
    RuntimeHealthReport report({
      required RuntimeVersionCompatibility compatibility,
      String? normalized,
      String? acknowledged,
    }) {
      return RuntimeHealthReport(
        schemaVersion: 1,
        providerId: 'ollama',
        displayName: 'Local AI runtime',
        state: RuntimeState.degraded,
        ownership: RuntimeOwnership.external,
        reuseConsent: RuntimeConsentState.reuseApproved,
        managementConsent: RuntimeConsentState.notRequested,
        endpointSafety: RuntimeEndpointSafety.loopbackVerified,
        version: RuntimeVersionInfo(
          reportedVersion: normalized ?? 'unknown',
          normalizedVersion: normalized,
          compatibility: compatibility,
        ),
        acknowledgedUntestedVersion: acknowledged,
        capabilities: const [],
        reasons: const [],
        warnings: const [],
      );
    }

    testWidgets('an untested version offers an explicit acknowledgement', (
      tester,
    ) async {
      String? accepted;
      await tester.pumpWidget(
        _host(
          RuntimePanel(
            state: RuntimeStatusLoaded(
              report: report(
                compatibility: RuntimeVersionCompatibility.untested,
                normalized: '0.32.14',
              ),
            ),
            inventoryState: const RuntimeInventoryIdle(),
            onAcknowledgeUntestedVersion: (version) => accepted = version,
          ),
        ),
      );

      await tester.tap(find.byKey(AppKeys.runtimeVersionAcknowledgeAction));
      await tester.pumpAndSettle();
      // The dialog must name the exact version being accepted.
      expect(find.textContaining('0.32.14'), findsWidgets);
      await tester.tap(find.text('Accept this version'));
      await tester.pumpAndSettle();

      expect(accepted, '0.32.14');
    });

    testWidgets('cancelling the dialog accepts nothing', (tester) async {
      String? accepted;
      await tester.pumpWidget(
        _host(
          RuntimePanel(
            state: RuntimeStatusLoaded(
              report: report(
                compatibility: RuntimeVersionCompatibility.untested,
                normalized: '0.32.14',
              ),
            ),
            inventoryState: const RuntimeInventoryIdle(),
            onAcknowledgeUntestedVersion: (version) => accepted = version,
          ),
        ),
      );

      await tester.tap(find.byKey(AppKeys.runtimeVersionAcknowledgeAction));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Not now'));
      await tester.pumpAndSettle();

      expect(accepted, isNull);
    });

    testWidgets('an already-acknowledged version offers no action', (
      tester,
    ) async {
      await tester.pumpWidget(
        _host(
          RuntimePanel(
            state: RuntimeStatusLoaded(
              report: report(
                compatibility: RuntimeVersionCompatibility.untested,
                normalized: '0.32.14',
                acknowledged: '0.32.14',
              ),
            ),
            inventoryState: const RuntimeInventoryIdle(),
            onAcknowledgeUntestedVersion: (_) {},
          ),
        ),
      );

      expect(find.byKey(AppKeys.runtimeVersionAcknowledgeAction), findsNothing);
    });

    testWidgets('a stale acknowledgement asks again after an update', (
      tester,
    ) async {
      await tester.pumpWidget(
        _host(
          RuntimePanel(
            state: RuntimeStatusLoaded(
              report: report(
                compatibility: RuntimeVersionCompatibility.untested,
                normalized: '0.33.0',
                acknowledged: '0.32.14',
              ),
            ),
            inventoryState: const RuntimeInventoryIdle(),
            onAcknowledgeUntestedVersion: (_) {},
          ),
        ),
      );

      expect(
        find.byKey(AppKeys.runtimeVersionAcknowledgeAction),
        findsOneWidget,
      );
    });

    testWidgets('a compatible version is never offered acknowledgement', (
      tester,
    ) async {
      await tester.pumpWidget(
        _host(
          RuntimePanel(
            state: RuntimeStatusLoaded(
              report: report(
                compatibility: RuntimeVersionCompatibility.compatible,
                normalized: '0.32.5',
              ),
            ),
            inventoryState: const RuntimeInventoryIdle(),
            onAcknowledgeUntestedVersion: (_) {},
          ),
        ),
      );

      expect(find.byKey(AppKeys.runtimeVersionAcknowledgeAction), findsNothing);
    });
  });
}
