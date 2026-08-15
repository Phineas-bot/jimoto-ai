import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gixgiz_desktop/app/app_keys.dart';
import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';
import 'package:gixgiz_desktop/features/foundation/foundation_state.dart';
import 'package:gixgiz_desktop/features/foundation/setup_panel.dart';
import 'package:gixgiz_desktop/l10n/app_localizations.dart';

import 'support/setup_fixture.dart';

void main() {
  testWidgets('approval echoes the persisted plan and exact effects', (
    tester,
  ) async {
    var approved = false;
    final job = setupJobFixture(
      state: 'awaiting_approval',
      stage: 'awaiting_approval',
    );

    await _pumpPanel(
      tester,
      SetupWorkflowAwaitingApproval(job: job),
      onApprove: () => approved = true,
    );

    expect(find.text('Fixture model'), findsOneWidget);
    expect(find.text('fixture:latest'), findsOneWidget);
    expect(find.text('Provider-managed local storage'), findsOneWidget);
    expect(find.text('Apache-2.0'), findsOneWidget);
    expect(find.text('Add provider-owned model data'), findsWidgets);
    expect(find.text('Selected local model (required)'), findsOneWidget);
    expect(
      find.text(
        'This model comes from the deterministic recommendation shown above.',
      ),
      findsOneWidget,
    );
    expect(
      find.text('The local runtime owns and controls this model storage.'),
      findsOneWidget,
    );
    expect(
      find.text('Selected by the deterministic capability report.'),
      findsNothing,
    );
    expect(find.text('The provider owns this storage location.'), findsNothing);
    expect(find.text('Decline plan'), findsOneWidget);

    await tester.ensureVisible(find.byKey(AppKeys.setupApproveAction));
    await tester.tap(find.byKey(AppKeys.setupApproveAction));
    await tester.pump();
    expect(find.text('Approve this model setup?'), findsOneWidget);
    expect(find.text('Not now'), findsOneWidget);
    expect(
      find.textContaining('This exact approval covers 2.0 GiB'),
      findsOneWidget,
    );

    await tester.tap(find.widgetWithText(FilledButton, 'Approve plan').last);
    await tester.pump();
    expect(approved, isTrue);
  });

  testWidgets('active progress is semantic and cancellation is explicit', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    var cancelled = false;
    final job = setupJobFixture(
      state: 'active',
      stage: 'acquiring',
      progress: const {
        'phase': 'transferring',
        'completed_bytes': 1073741824,
        'total_bytes': 2147483648,
        'progress_basis_points': 5000,
      },
    );

    await _pumpPanel(
      tester,
      SetupWorkflowActive(job: job),
      onCancel: () => cancelled = true,
    );

    final progress = tester
        .getSemantics(find.byKey(AppKeys.setupProgress))
        .getSemanticsData();
    expect(progress.label, 'Model setup progress');
    expect(progress.value, '1.0 GiB of 2.0 GiB');

    await tester.ensureVisible(find.byKey(AppKeys.setupCancelAction));
    await tester.tap(find.byKey(AppKeys.setupCancelAction));
    await tester.pump();
    expect(find.text('Cancel model setup?'), findsOneWidget);
    expect(find.text('Keep running'), findsOneWidget);
    expect(cancelled, isFalse);
    await tester.tap(find.widgetWithText(FilledButton, 'Cancel setup').last);
    await tester.pump();
    expect(cancelled, isTrue);
    semantics.dispose();
  });

  testWidgets('approved and attention cancellation require confirmation', (
    tester,
  ) async {
    final cases = <(SetupWorkflowState, String)>[
      (
        SetupWorkflowApproved(
          job: setupJobFixture(state: 'approved', stage: 'approved'),
        ),
        'Cancel this approved setup before it starts. No provider work will begin.',
      ),
      (
        SetupWorkflowAttention(
          job: setupJobFixture(
            state: 'attention_required',
            stage: 'attention_required',
          ),
        ),
        'Cancel this setup after reviewing its attention state. Existing completed, retained, rolled-back, and uncertain effects will remain recorded.',
      ),
    ];

    for (final (state, expectedMessage) in cases) {
      var cancellations = 0;
      await _pumpPanel(tester, state, onCancel: () => cancellations += 1);

      await tester.ensureVisible(find.byKey(AppKeys.setupCancelAction));
      await tester.tap(find.byKey(AppKeys.setupCancelAction));
      await tester.pump();
      expect(cancellations, 0);
      expect(find.text(expectedMessage), findsOneWidget);

      await tester.tap(find.widgetWithText(TextButton, 'Keep running'));
      await tester.pump();
      expect(cancellations, 0);

      await tester.tap(find.byKey(AppKeys.setupCancelAction));
      await tester.pump();
      await tester.tap(find.widgetWithText(FilledButton, 'Cancel setup').last);
      await tester.pump();
      expect(cancellations, 1);
    }
  });

  testWidgets('stable reason and warning codes drive localized copy', (
    tester,
  ) async {
    final job = setupJobFixture(
      reasons: const [
        {'code': 'existing_model_reusable', 'message': 'RAW_REASON_1'},
        {'code': 'acquisition_required', 'message': 'RAW_REASON_2'},
        {'code': 'approval_required', 'message': 'RAW_REASON_3'},
        {'code': 'storage_verified', 'message': 'RAW_REASON_4'},
        {'code': 'runtime_verified', 'message': 'RAW_REASON_5'},
        {'code': 'registration_verified', 'message': 'RAW_REASON_6'},
        {'code': 'readiness_verified', 'message': 'RAW_REASON_7'},
        {'code': 'future_reason', 'message': 'RAW_REASON_8'},
      ],
      warnings: const [
        {'code': 'external_runtime_modified', 'message': 'RAW_WARNING_1'},
        {'code': 'provider_managed_storage', 'message': 'RAW_WARNING_2'},
        {'code': 'integrity_metadata_unavailable', 'message': 'RAW_WARNING_3'},
        {'code': 'cancellation_may_retain_effects', 'message': 'RAW_WARNING_4'},
        {'code': 'destination_evidence_incomplete', 'message': 'RAW_WARNING_5'},
        {'code': 'future_warning', 'message': 'RAW_WARNING_6'},
      ],
    );

    await _pumpPanel(tester, SetupWorkflowAwaitingApproval(job: job));

    expect(
      find.text(
        'The exact provider model is already available and may be reused.',
      ),
      findsOneWidget,
    );
    expect(
      find.text(
        'The selected provider model must be added before verification.',
      ),
      findsOneWidget,
    );
    expect(
      find.text(
        'The listed local effects require approval before setup starts.',
      ),
      findsOneWidget,
    );
    expect(
      find.text('The bounded readiness check was verified.'),
      findsOneWidget,
    );
    expect(
      find.text('The core reported an unrecognized setup reason.'),
      findsOneWidget,
    );
    expect(
      find.textContaining('without transferring its ownership to GixGiz'),
      findsOneWidget,
    );
    expect(
      find.text(
        'No trusted independent checksum is available for this provider model.',
      ),
      findsOneWidget,
    );
    expect(
      find.text('The core reported an unrecognized setup warning.'),
      findsOneWidget,
    );
    expect(find.textContaining('RAW_REASON_'), findsNothing);
    expect(find.textContaining('RAW_WARNING_'), findsNothing);
  });

  testWidgets('ready and retained effects come only from persisted state', (
    tester,
  ) async {
    final job = setupJobFixture(
      state: 'ready',
      stage: 'ready',
      effectReport: const {
        'effects': [
          {
            'kind': 'provider_model_acquisition',
            'disposition': 'retained',
            'message': 'Provider-owned model data remains available.',
          },
          {
            'kind': 'readiness_inference',
            'disposition': 'completed',
            'message': 'The bounded readiness check succeeded.',
          },
        ],
      },
    );

    await _pumpPanel(
      tester,
      SetupWorkflowReady(job: job),
      textScaler: const TextScaler.linear(2),
    );

    expect(find.text('Local model ready'), findsOneWidget);
    expect(find.textContaining('Retained: Add provider-owned'), findsOneWidget);
    expect(find.textContaining('Completed: Run a bounded'), findsOneWidget);
    expect(
      find.text('Provider-owned model data remains available.'),
      findsNothing,
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'preparing acquiring and verification stages expose live semantics',
    (tester) async {
      final semantics = tester.ensureSemantics();
      const stages = <(String, String)>[
        ('preparing', 'Preparing model setup'),
        ('acquiring', 'Acquiring model'),
        ('verifying_runtime', 'Verifying runtime'),
        ('verifying_model', 'Verifying model'),
        ('running_test_inference', 'Testing readiness'),
      ];

      for (final (stage, expectedTitle) in stages) {
        await _pumpPanel(
          tester,
          SetupWorkflowActive(
            job: setupJobFixture(state: 'active', stage: stage),
          ),
        );

        final status = tester
            .getSemantics(find.byKey(AppKeys.setupStatus))
            .getSemanticsData();
        expect(status.label, contains(expectedTitle));
      }

      semantics.dispose();
    },
  );

  testWidgets(
    'attention failed cancelled and ready states remain actionable and typed',
    (tester) async {
      final semantics = tester.ensureSemantics();
      var retries = 0;
      final states = <(SetupWorkflowState, String, bool)>[
        (
          SetupWorkflowAttention(
            job: setupJobFixture(
              state: 'attention_required',
              stage: 'attention_required',
            ),
          ),
          'Setup needs attention',
          true,
        ),
        (
          SetupWorkflowFailed(
            diagnosticCode: 'SETUP_FIXTURE_FAILED',
            job: setupJobFixture(state: 'failed', stage: 'failed'),
            recoveryAction: SetupRecoveryAction.retry,
          ),
          'Model setup failed',
          true,
        ),
        (
          SetupWorkflowCancelled(
            job: setupJobFixture(state: 'cancelled', stage: 'cancelled'),
          ),
          'Model setup cancelled',
          true,
        ),
        (
          SetupWorkflowReady(
            job: setupJobFixture(state: 'ready', stage: 'ready'),
          ),
          'Local model ready',
          false,
        ),
      ];

      for (final (state, expectedTitle, retryable) in states) {
        await _pumpPanel(
          tester,
          state,
          onRetry: retryable ? () => retries += 1 : null,
        );

        final status = tester
            .getSemantics(find.byKey(AppKeys.setupStatus))
            .getSemanticsData();
        expect(status.label, contains(expectedTitle));
        expect(
          find.byKey(AppKeys.setupRetryAction),
          retryable ? findsOneWidget : findsNothing,
        );
      }

      expect(retries, 0);
      semantics.dispose();
    },
  );

  testWidgets('transport recovery overrides stale persisted guidance', (
    tester,
  ) async {
    final job = setupJobFixture(
      state: 'attention_required',
      stage: 'attention_required',
    );

    await _pumpPanel(
      tester,
      SetupWorkflowFailed(
        diagnosticCode: 'SETUP_TRANSPORT_FAILED',
        job: job,
        recoveryAction: job.recoveryAction,
        transportCategory: ErrorCategory.unavailable,
        transportRecoveryAction: RecoveryAction.contactSupport,
        transportRecoveryMessage: 'Use diagnostic code SETUP_TRANSPORT_FAILED.',
      ),
    );

    expect(
      find.text(
        'Recommended action: Contact support with the diagnostic code.',
      ),
      findsOneWidget,
    );
    expect(
      find.text(
        'Recommended action: Retry after GixGiz rechecks current conditions.',
      ),
      findsNothing,
    );

    await tester.ensureVisible(find.byKey(AppKeys.setupDiagnostics));
    await tester.tap(find.byKey(AppKeys.setupDiagnostics));
    await tester.pumpAndSettle();
    expect(
      find.text('Use diagnostic code SETUP_TRANSPORT_FAILED.'),
      findsOneWidget,
    );
  });

  testWidgets('persisted failure renders its safe recovery guidance', (
    tester,
  ) async {
    final failedJson = setupJobFixture(
      state: 'failed',
      stage: 'failed',
    ).toJson();
    failedJson['recovery_action'] = null;
    failedJson['error'] = {
      'category': 'integrity_failure',
      'code': 'setup.integrity_mismatch',
      'correlation_id': setupCorrelationIdFixture,
      'message': 'Model integrity evidence did not match.',
      'recovery': {
        'action': 'contact_support',
        'message': 'Review the diagnostic code before creating a new plan.',
      },
      'request_id': '00000000-0000-4000-8000-000000000012',
    };
    final job = SetupJobSnapshot.fromJson(failedJson);

    await _pumpPanel(
      tester,
      SetupWorkflowFailed(
        diagnosticCode: job.error!.code,
        job: job,
        recoveryAction: job.recoveryAction,
      ),
    );

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
      find.text('Review the diagnostic code before creating a new plan.'),
      findsOneWidget,
    );
  });
}

Future<void> _pumpPanel(
  WidgetTester tester,
  SetupWorkflowState state, {
  VoidCallback? onApprove,
  VoidCallback? onCancel,
  VoidCallback? onRetry,
  TextScaler textScaler = TextScaler.noScaling,
}) async {
  await tester.pumpWidget(
    MediaQuery(
      data: MediaQueryData(textScaler: textScaler),
      child: MaterialApp(
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: Scaffold(
          body: SingleChildScrollView(
            padding: const EdgeInsets.all(24),
            child: SetupPanel(
              state: state,
              onApprove: onApprove,
              onCancel: onCancel,
              onRetry: onRetry,
            ),
          ),
        ),
      ),
    ),
  );
  await tester.pump();
}
