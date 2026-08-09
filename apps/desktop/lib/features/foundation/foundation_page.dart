import 'dart:async';

import 'package:flutter/material.dart';
import 'package:gixgiz_desktop/core/core_client.dart';
import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';
import 'package:gixgiz_desktop/features/foundation/foundation_screen.dart';
import 'package:gixgiz_desktop/features/foundation/foundation_state.dart';

class FoundationPage extends StatefulWidget {
  const FoundationPage({required this.coreClient, super.key});

  final CoreClient coreClient;

  @override
  State<FoundationPage> createState() => _FoundationPageState();
}

class _FoundationPageState extends State<FoundationPage> {
  FoundationState _state = const FoundationLoading();
  HardwareScanState _hardwareScanState = const HardwareScanIdle();
  CapabilityRecommendationState _recommendationState =
      const CapabilityRecommendationIdle();
  UserPreferenceProfile _preferences = const UserPreferenceProfile(
    workload: WorkloadTier.generalText,
    priority: PreferencePriority.balanced,
    includeOptionalLarger: true,
  );
  CoreOperation? _activeHardwareScan;
  CoreClient? _activeHardwareScanClient;
  StreamSubscription<HardwareScanEvent>? _hardwareScanSubscription;

  @override
  void initState() {
    super.initState();
    _checkConnection();
  }

  @override
  void didUpdateWidget(FoundationPage oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(oldWidget.coreClient, widget.coreClient)) {
      unawaited(_stopHardwareScan(cancelRemote: true));
      _hardwareScanState = const HardwareScanIdle();
      _recommendationState = const CapabilityRecommendationIdle();
      _checkConnection();
    }
  }

  @override
  void dispose() {
    unawaited(_stopHardwareScan(cancelRemote: true));
    super.dispose();
  }

  Future<void> _checkConnection() async {
    if (!mounted) {
      return;
    }
    setState(() => _state = const FoundationLoading());

    try {
      final snapshot = await widget.coreClient.checkConnection();
      if (!mounted) {
        return;
      }
      setState(() => _state = _mapSnapshot(snapshot));
    } on Object {
      if (!mounted) {
        return;
      }
      setState(
        () => _state = const FoundationFailed(
          issue: CoreConnectionIssue.internalFailure,
          diagnosticCode: 'CORE_CHECK_FAILED',
        ),
      );
    }
  }

  FoundationState _mapSnapshot(CoreConnectionSnapshot snapshot) {
    return switch (snapshot.kind) {
      CoreConnectionKind.ready => FoundationReady(snapshot: snapshot),
      CoreConnectionKind.degraded => FoundationDegraded(snapshot: snapshot),
      CoreConnectionKind.unavailable => FoundationUnavailable(
        issue: snapshot.issue,
        diagnosticCode: snapshot.diagnosticCode ?? 'CORE_UNAVAILABLE',
      ),
      CoreConnectionKind.failed => FoundationFailed(
        issue: snapshot.issue,
        diagnosticCode: snapshot.diagnosticCode ?? 'CORE_UNAVAILABLE',
      ),
      CoreConnectionKind.cancelled => const FoundationCancelled(),
    };
  }

  Future<void> _startHardwareScan() async {
    await _stopHardwareScan(cancelRemote: false);
    if (!mounted) {
      return;
    }
    setState(() {
      _hardwareScanState = const HardwareScanLoading();
      _recommendationState = const CapabilityRecommendationIdle();
    });
    final client = widget.coreClient;
    try {
      final operation = await client.startHardwareScan();
      if (!mounted || !identical(client, widget.coreClient)) {
        unawaited(_cancelQuietly(client, operation));
        return;
      }
      _activeHardwareScan = operation;
      _activeHardwareScanClient = client;
      _hardwareScanSubscription = client
          .observeHardwareScan(operation)
          .listen(
            _applyHardwareScanEvent,
            onError: (_) => _failHardwareScan('HARDWARE_SCAN_STREAM_FAILED'),
            onDone: () {
              if (_hardwareScanState is HardwareScanLoading) {
                _failHardwareScan('HARDWARE_SCAN_STREAM_ENDED');
              }
            },
          );
    } on Object {
      _failHardwareScan('HARDWARE_SCAN_START_FAILED');
    }
  }

  void _applyHardwareScanEvent(HardwareScanEvent event) {
    if (!mounted || event.terminalState == null) {
      return;
    }
    _activeHardwareScan = null;
    _activeHardwareScanClient = null;
    final profile = event.profile;
    final next = switch (event.terminalState!) {
      HardwareScanTerminalState.completed when profile != null =>
        HardwareScanReady(profile: profile),
      HardwareScanTerminalState.partial when profile != null =>
        HardwareScanPartial(profile: profile),
      HardwareScanTerminalState.cancelled => const HardwareScanCancelled(),
      HardwareScanTerminalState.timedOut => HardwareScanFailed(
        diagnosticCode: event.error?.code ?? 'HARDWARE_SCAN_TIMED_OUT',
      ),
      HardwareScanTerminalState.failed => HardwareScanFailed(
        diagnosticCode: event.error?.code ?? 'HARDWARE_SCAN_FAILED',
      ),
      _ => const HardwareScanFailed(
        diagnosticCode: 'HARDWARE_SCAN_RESPONSE_INVALID',
      ),
    };
    setState(() => _hardwareScanState = next);
  }

  void _setWorkload(WorkloadTier workload) {
    setState(() {
      _preferences = UserPreferenceProfile(
        workload: workload,
        priority: _preferences.priority,
        includeOptionalLarger: _preferences.includeOptionalLarger,
      );
      _recommendationState = const CapabilityRecommendationIdle();
    });
  }

  void _setPriority(PreferencePriority priority) {
    setState(() {
      _preferences = UserPreferenceProfile(
        workload: _preferences.workload,
        priority: priority,
        includeOptionalLarger: _preferences.includeOptionalLarger,
      );
      _recommendationState = const CapabilityRecommendationIdle();
    });
  }

  void _setIncludeOptionalLarger(bool include) {
    setState(() {
      _preferences = UserPreferenceProfile(
        workload: _preferences.workload,
        priority: _preferences.priority,
        includeOptionalLarger: include,
      );
      _recommendationState = const CapabilityRecommendationIdle();
    });
  }

  Future<void> _generateRecommendation() async {
    final profile = switch (_hardwareScanState) {
      HardwareScanReady(:final profile) ||
      HardwareScanPartial(:final profile) => profile,
      _ => null,
    };
    if (profile == null) {
      return;
    }
    final client = widget.coreClient;
    setState(
      () => _recommendationState = const CapabilityRecommendationLoading(),
    );
    try {
      final report = await client.recommendCapability(profile, _preferences);
      if (!mounted || !identical(client, widget.coreClient)) {
        return;
      }
      setState(() {
        _recommendationState = switch (report.status) {
          CapabilityReportStatus.plansAvailable =>
            CapabilityRecommendationReady(report: report),
          CapabilityReportStatus.noPlan || CapabilityReportStatus.unknown =>
            CapabilityRecommendationNoPlan(report: report),
        };
      });
    } on Object {
      if (!mounted || !identical(client, widget.coreClient)) {
        return;
      }
      setState(
        () => _recommendationState = const CapabilityRecommendationFailed(
          diagnosticCode: 'CAPABILITY_RECOMMENDATION_FAILED',
        ),
      );
    }
  }

  void _failHardwareScan(String diagnosticCode) {
    if (!mounted) {
      return;
    }
    _activeHardwareScan = null;
    _activeHardwareScanClient = null;
    setState(
      () => _hardwareScanState = HardwareScanFailed(
        diagnosticCode: diagnosticCode,
      ),
    );
  }

  Future<void> _cancelHardwareScan() async {
    final operation = _activeHardwareScan;
    final client = _activeHardwareScanClient;
    if (operation == null || client == null) {
      return;
    }
    try {
      await client.cancelHardwareScan(operation);
    } on Object {
      _failHardwareScan('HARDWARE_SCAN_CANCEL_FAILED');
    }
  }

  Future<void> _stopHardwareScan({required bool cancelRemote}) async {
    final operation = _activeHardwareScan;
    final client = _activeHardwareScanClient;
    _activeHardwareScan = null;
    _activeHardwareScanClient = null;
    await _hardwareScanSubscription?.cancel();
    _hardwareScanSubscription = null;
    if (cancelRemote && operation != null && client != null) {
      unawaited(_cancelQuietly(client, operation));
    }
  }

  Future<void> _cancelQuietly(
    CoreClient client,
    CoreOperation operation,
  ) async {
    try {
      await client.cancelHardwareScan(operation);
    } on Object {
      // Teardown is already in progress; the supervised sidecar owns cleanup.
    }
  }

  @override
  Widget build(BuildContext context) {
    return FoundationScreen(
      state: _state,
      hardwareScanState: _hardwareScanState,
      recommendationState: _recommendationState,
      preferences: _preferences,
      onRetry: _checkConnection,
      onStartHardwareScan: _startHardwareScan,
      onCancelHardwareScan: _cancelHardwareScan,
      onWorkloadChanged: _setWorkload,
      onPriorityChanged: _setPriority,
      onIncludeOptionalLargerChanged: _setIncludeOptionalLarger,
      onGenerateRecommendation: _generateRecommendation,
    );
  }
}
