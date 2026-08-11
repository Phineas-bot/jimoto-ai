import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gixgiz_desktop/app/app_keys.dart';
import 'package:gixgiz_desktop/app/app_theme.dart';
import 'package:gixgiz_desktop/app/gixgiz_app.dart';
import 'package:gixgiz_desktop/core/core_client.dart';
import 'package:gixgiz_desktop/features/foundation/foundation_screen.dart';
import 'package:gixgiz_desktop/features/foundation/foundation_state.dart';
import 'package:gixgiz_desktop/l10n/app_localizations.dart';

import 'support/stub_core_client.dart';

void main() {
  testWidgets('navigation switches between Foundation and About', (
    tester,
  ) async {
    await tester.pumpWidget(
      const GixGizApp(
        coreClient: StubCoreClient(
          CoreConnectionSnapshot(kind: CoreConnectionKind.ready),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(AppKeys.aboutNavigation));
    await tester.pumpAndSettle();

    expect(find.text('About GixGiz'), findsOneWidget);
    expect(find.text('ai.gixgiz.desktop'), findsOneWidget);

    await tester.tap(find.byKey(AppKeys.foundationNavigation));
    await tester.pumpAndSettle();

    expect(find.text('Foundation'), findsWidgets);
  });

  testWidgets('keyboard activates rail navigation destination', (tester) async {
    tester.view.physicalSize = const Size(1200, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(
      const GixGizApp(
        coreClient: StubCoreClient(
          CoreConnectionSnapshot(kind: CoreConnectionKind.ready),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();

    expect(find.text('About GixGiz'), findsOneWidget);
  });

  testWidgets('keyboard traversal reaches action with visible focus styling', (
    tester,
  ) async {
    final actionFocusNode = FocusNode();
    addTearDown(actionFocusNode.dispose);

    await _pumpFoundation(
      tester,
      const FoundationUnavailable(
        issue: CoreConnectionIssue.coreUnavailable,
        diagnosticCode: 'CORE_NOT_CONNECTED',
      ),
      actionFocusNode: actionFocusNode,
    );

    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    expect(actionFocusNode.hasFocus, isFalse);

    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();

    expect(actionFocusNode.hasFocus, isTrue);
    final context = tester.element(find.byKey(AppKeys.primaryAction));
    final focusedSide = FilledButtonTheme.of(
      context,
    ).style?.side?.resolve({WidgetState.focused});
    expect(focusedSide?.width, greaterThanOrEqualTo(2));
  });

  testWidgets(
    'runtime refresh supports keyboard activation and visible focus',
    (tester) async {
      final actionFocusNode = FocusNode();
      addTearDown(actionFocusNode.dispose);
      var refreshed = false;

      await _pumpFoundation(
        tester,
        const FoundationReady(
          snapshot: CoreConnectionSnapshot(kind: CoreConnectionKind.ready),
        ),
        runtimeActionFocusNode: actionFocusNode,
        onRefreshRuntime: () => refreshed = true,
      );

      actionFocusNode.requestFocus();
      await tester.pump();

      expect(actionFocusNode.hasFocus, isTrue);
      final button = tester.widget<OutlinedButton>(
        find.byKey(AppKeys.runtimeRefreshAction),
      );
      final focusedSide = button.style?.side?.resolve({WidgetState.focused});
      expect(focusedSide?.width, greaterThanOrEqualTo(2));

      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pump();
      expect(refreshed, isTrue);
    },
  );

  testWidgets('primary status and recovery action expose semantics', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    try {
      await _pumpFoundation(
        tester,
        const FoundationUnavailable(
          issue: CoreConnectionIssue.coreUnavailable,
          diagnosticCode: 'CORE_NOT_CONNECTED',
        ),
      );

      final statusData = tester
          .getSemantics(find.byKey(AppKeys.foundationStatus))
          .getSemanticsData();
      expect(
        statusData.label,
        contains('Foundation status: Core not connected'),
      );

      final actionData = tester
          .getSemantics(find.byKey(AppKeys.primaryAction))
          .getSemanticsData();
      expect(actionData.label, 'Check again');
      expect(actionData.hasAction(SemanticsAction.tap), isTrue);
    } finally {
      semantics.dispose();
    }
  });

  testWidgets('foundation layout tolerates doubled text scale', (tester) async {
    tester.view.physicalSize = const Size(1200, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await _pumpFoundation(
      tester,
      const FoundationFailed(
        issue: CoreConnectionIssue.internalFailure,
        diagnosticCode: 'CORE_TEST_FAILURE',
      ),
      textScaler: const TextScaler.linear(2),
    );

    expect(tester.takeException(), isNull);
    expect(find.text('Foundation check failed'), findsOneWidget);
  });
}

Future<void> _pumpFoundation(
  WidgetTester tester,
  FoundationState state, {
  FocusNode? actionFocusNode,
  FocusNode? runtimeActionFocusNode,
  VoidCallback? onRefreshRuntime,
  TextScaler textScaler = TextScaler.noScaling,
}) async {
  await tester.pumpWidget(
    MediaQuery(
      data: MediaQueryData(textScaler: textScaler),
      child: MaterialApp(
        theme: AppTheme.light,
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: Scaffold(
          body: FoundationScreen(
            state: state,
            hardwareScanState: const HardwareScanIdle(),
            onRetry: () {},
            onStartHardwareScan: () {},
            onCancelHardwareScan: () {},
            onRefreshRuntime: onRefreshRuntime,
            runtimeActionFocusNode: runtimeActionFocusNode,
            primaryActionFocusNode: actionFocusNode,
          ),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
}
