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
  RuntimeStatusState _runtimeStatusState = const RuntimeStatusIdle();
  RuntimeInventoryState _runtimeInventoryState = const RuntimeInventoryIdle();
  SetupWorkflowState _setupState = const SetupWorkflowIdle();
  UserPreferenceProfile _preferences = const UserPreferenceProfile(
    workload: WorkloadTier.generalText,
    priority: PreferencePriority.balanced,
    includeOptionalLarger: true,
  );
  CoreOperation? _activeHardwareScan;
  CoreClient? _activeHardwareScanClient;
  StreamSubscription<HardwareScanEvent>? _hardwareScanSubscription;
  CoreOperation? _activeRuntimeOperation;
  CoreClient? _activeRuntimeClient;
  StreamSubscription<RuntimeOperationEvent>? _runtimeOperationSubscription;
  int _runtimeRevision = 0;
  SetupJobSnapshot? _setupJob;
  CoreClient? _setupStreamClient;
  SetupJobId? _setupStreamJobId;
  StreamSubscription<SetupJobEvent>? _setupSubscription;
  Timer? _setupReconnectTimer;
  int _setupAfterSequence = 0;
  int _setupRevision = 0;

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
      unawaited(_stopRuntimeOperation(cancelRemote: true));
      unawaited(_detachSetupStream());
      _hardwareScanState = const HardwareScanIdle();
      _recommendationState = const CapabilityRecommendationIdle();
      _runtimeStatusState = const RuntimeStatusIdle();
      _runtimeInventoryState = const RuntimeInventoryIdle();
      _setupState = const SetupWorkflowIdle();
      _setupJob = null;
      _setupAfterSequence = 0;
      _setupRevision += 1;
      _runtimeRevision += 1;
      _checkConnection();
    }
  }

  @override
  void dispose() {
    unawaited(_stopHardwareScan(cancelRemote: true));
    unawaited(_stopRuntimeOperation(cancelRemote: true));
    unawaited(_detachSetupStream());
    super.dispose();
  }

  Future<void> _checkConnection() async {
    if (!mounted) {
      return;
    }
    setState(() => _state = const FoundationLoading());

    final client = widget.coreClient;
    try {
      final snapshot = await client.checkConnection();
      if (!mounted || !identical(client, widget.coreClient)) {
        return;
      }
      final next = _mapSnapshot(snapshot);
      setState(() => _state = next);
      if ((next is FoundationReady || next is FoundationDegraded) &&
          client.supportsTransportCapability(
            TransportCapability.runtimeStatus,
          )) {
        unawaited(_checkRuntimeStatus());
      }
      if ((next is FoundationReady || next is FoundationDegraded) &&
          client.supportsTransportCapability(
            TransportCapability.setupWorkflow,
          )) {
        unawaited(_recoverSetupJob());
      }
    } on Object {
      if (!mounted || !identical(client, widget.coreClient)) {
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

  RuntimeHealthReport? _runtimeReport() {
    return switch (_runtimeStatusState) {
      RuntimeStatusLoading(:final previousReport) => previousReport,
      RuntimeStatusLoaded(:final report) ||
      RuntimeStatusOperating(:final report) => report,
      RuntimeStatusFailed(:final report) ||
      RuntimeStatusCancelled(:final report) => report,
      RuntimeStatusIdle() => null,
    };
  }

  Future<void> _checkRuntimeStatus() async {
    if (!mounted || _activeRuntimeOperation != null) {
      return;
    }
    final client = widget.coreClient;
    final previousReport = _runtimeReport();
    final revision = ++_runtimeRevision;
    setState(() {
      _runtimeStatusState = RuntimeStatusLoading(
        previousReport: previousReport,
      );
      _runtimeInventoryState = const RuntimeInventoryIdle();
    });
    try {
      final report = await client.checkRuntimeStatus();
      if (!mounted ||
          !identical(client, widget.coreClient) ||
          revision != _runtimeRevision) {
        return;
      }
      setState(() => _runtimeStatusState = RuntimeStatusLoaded(report: report));
    } on CoreClientFailure catch (failure) {
      if (!mounted ||
          !identical(client, widget.coreClient) ||
          revision != _runtimeRevision) {
        return;
      }
      setState(() {
        _runtimeStatusState = RuntimeStatusFailed(
          diagnosticCode: failure.code,
          report: previousReport,
          recoveryAction: failure.recoveryAction,
        );
      });
    } on Object {
      if (!mounted ||
          !identical(client, widget.coreClient) ||
          revision != _runtimeRevision) {
        return;
      }
      setState(() {
        _runtimeStatusState = RuntimeStatusFailed(
          diagnosticCode: 'RUNTIME_STATUS_FAILED',
          report: previousReport,
        );
      });
    }
  }

  Future<void> _approveRuntimeReuse() async {
    final report = _runtimeReport();
    if (report == null || _activeRuntimeOperation != null) {
      return;
    }
    final client = widget.coreClient;
    final revision = ++_runtimeRevision;
    setState(() {
      _runtimeStatusState = RuntimeStatusLoading(previousReport: report);
      _runtimeInventoryState = const RuntimeInventoryIdle();
    });
    try {
      final updated = await client.decideRuntimeReuse(
        RuntimeConsentDecision.approveReuse,
      );
      if (!mounted ||
          !identical(client, widget.coreClient) ||
          revision != _runtimeRevision) {
        return;
      }
      setState(() {
        _runtimeStatusState = RuntimeStatusLoaded(report: updated);
        _runtimeInventoryState = const RuntimeInventoryIdle();
      });
    } on CoreClientFailure catch (failure) {
      if (!mounted ||
          !identical(client, widget.coreClient) ||
          revision != _runtimeRevision) {
        return;
      }
      setState(() {
        _runtimeStatusState = RuntimeStatusFailed(
          diagnosticCode: failure.code,
          report: report,
          recoveryAction: failure.recoveryAction,
        );
      });
    } on Object {
      if (!mounted ||
          !identical(client, widget.coreClient) ||
          revision != _runtimeRevision) {
        return;
      }
      setState(() {
        _runtimeStatusState = RuntimeStatusFailed(
          diagnosticCode: 'RUNTIME_CONSENT_FAILED',
          report: report,
        );
      });
    }
  }

  Future<void> _startRuntimeOperation(RuntimeOperationKind kind) async {
    final report = _runtimeReport();
    if (report == null || _activeRuntimeOperation != null) {
      return;
    }
    final client = widget.coreClient;
    _runtimeRevision += 1;
    setState(() {
      _runtimeStatusState = RuntimeStatusOperating(report: report, kind: kind);
      _runtimeInventoryState = const RuntimeInventoryIdle();
    });
    try {
      final operation = await client.startRuntimeOperation(kind);
      if (!mounted || !identical(client, widget.coreClient)) {
        unawaited(_cancelRuntimeQuietly(client, operation));
        return;
      }
      _activeRuntimeOperation = operation;
      _activeRuntimeClient = client;
      _runtimeOperationSubscription = client
          .observeRuntimeOperation(operation)
          .listen(
            (event) {
              if (_isActiveRuntimeOperation(client, operation)) {
                _applyRuntimeOperationEvent(event);
              }
            },
            onError: (Object error) {
              if (_isActiveRuntimeOperation(client, operation)) {
                _recoverRuntimeOperationStream(
                  client,
                  operation,
                  failure: error is CoreClientFailure ? error : null,
                  fallbackDiagnosticCode: 'RUNTIME_OPERATION_STREAM_FAILED',
                );
              }
            },
            onDone: () {
              if (_isActiveRuntimeOperation(client, operation) &&
                  _runtimeStatusState is RuntimeStatusOperating) {
                _recoverRuntimeOperationStream(client, operation);
              }
            },
          );
      setState(() {});
    } on CoreClientFailure catch (failure) {
      if (!mounted || !identical(client, widget.coreClient)) {
        return;
      }
      _handleRuntimeOperationFailure(failure, operationKind: kind);
    } on Object {
      if (!mounted || !identical(client, widget.coreClient)) {
        return;
      }
      _failRuntimeOperation(
        'RUNTIME_OPERATION_START_FAILED',
        operationKind: kind,
      );
    }
  }

  void _applyRuntimeOperationEvent(RuntimeOperationEvent event) {
    if (!mounted) {
      return;
    }
    final terminal = event.terminalState;
    if (terminal == null) {
      final report = event.report ?? _runtimeReport();
      if (report != null) {
        setState(() {
          _runtimeStatusState = RuntimeStatusOperating(
            report: report,
            kind: event.operationKind,
          );
        });
      }
      return;
    }
    final report = event.report;

    _activeRuntimeOperation = null;
    _activeRuntimeClient = null;
    setState(() {
      _runtimeInventoryState = const RuntimeInventoryIdle();
      _runtimeStatusState = switch (terminal) {
        RuntimeOperationTerminalState.completed when report != null =>
          RuntimeStatusLoaded(report: report),
        RuntimeOperationTerminalState.cancelled => RuntimeStatusCancelled(
          report: report,
        ),
        RuntimeOperationTerminalState.failed ||
        RuntimeOperationTerminalState.timedOut ||
        RuntimeOperationTerminalState.unknown => RuntimeStatusFailed(
          diagnosticCode:
              event.error?.code ?? 'RUNTIME_OPERATION_${terminal.wireValue}',
          report: report,
          operationKind: event.operationKind,
          recoveryAction: event.error?.recovery.action ?? RecoveryAction.retry,
        ),
        _ => RuntimeStatusLoading(previousReport: report),
      };
    });
    if (terminal == RuntimeOperationTerminalState.completed && report == null) {
      unawaited(_checkRuntimeStatus());
    }
  }

  void _recoverRuntimeOperationStream(
    CoreClient client,
    CoreOperation operation, {
    CoreClientFailure? failure,
    String fallbackDiagnosticCode = 'RUNTIME_OPERATION_STREAM_ENDED',
  }) {
    if (!mounted || !_isActiveRuntimeOperation(client, operation)) {
      return;
    }
    final operationKind = switch (_runtimeStatusState) {
      RuntimeStatusOperating(:final kind) => kind,
      _ => null,
    };
    final revision = ++_runtimeRevision;
    _activeRuntimeOperation = null;
    _activeRuntimeClient = null;
    final subscription = _runtimeOperationSubscription;
    _runtimeOperationSubscription = null;
    unawaited(subscription?.cancel());
    setState(() {
      _runtimeInventoryState = const RuntimeInventoryIdle();
      _runtimeStatusState = failure?.category == ErrorCategory.cancelled
          ? const RuntimeStatusCancelled()
          : RuntimeStatusFailed(
              diagnosticCode: failure?.code ?? fallbackDiagnosticCode,
              operationKind: operationKind,
              recoveryAction: failure?.recoveryAction ?? RecoveryAction.retry,
            );
    });
    unawaited(
      _cancelAndRefreshRuntimeStatus(client, operation, revision: revision),
    );
  }

  Future<void> _cancelAndRefreshRuntimeStatus(
    CoreClient client,
    CoreOperation operation, {
    required int revision,
  }) async {
    try {
      await client.cancelRuntimeOperation(operation);
    } on Object {
      // Status remains authoritative even when cancellation cannot be confirmed.
    }
    if (!mounted ||
        !identical(client, widget.coreClient) ||
        revision != _runtimeRevision ||
        !client.supportsTransportCapability(
          TransportCapability.runtimeStatus,
        )) {
      return;
    }
    try {
      final report = await client.checkRuntimeStatus();
      if (!mounted ||
          !identical(client, widget.coreClient) ||
          revision != _runtimeRevision) {
        return;
      }
      setState(() => _runtimeStatusState = RuntimeStatusLoaded(report: report));
    } on CoreClientFailure catch (failure) {
      if (!mounted ||
          !identical(client, widget.coreClient) ||
          revision != _runtimeRevision) {
        return;
      }
      setState(() {
        _runtimeStatusState = RuntimeStatusFailed(
          diagnosticCode: failure.code,
          recoveryAction: failure.recoveryAction,
        );
      });
    } on Object {
      if (!mounted ||
          !identical(client, widget.coreClient) ||
          revision != _runtimeRevision) {
        return;
      }
      setState(() {
        _runtimeStatusState = const RuntimeStatusFailed(
          diagnosticCode: 'RUNTIME_STATUS_FAILED',
        );
      });
    }
  }

  void _failRuntimeOperation(
    String diagnosticCode, {
    RecoveryAction recoveryAction = RecoveryAction.retry,
    RuntimeOperationKind? operationKind,
  }) {
    if (!mounted) {
      return;
    }
    final report = _runtimeReport();
    final failedKind =
        operationKind ??
        switch (_runtimeStatusState) {
          RuntimeStatusOperating(:final kind) => kind,
          RuntimeStatusFailed(:final operationKind) => operationKind,
          _ => null,
        };
    _activeRuntimeOperation = null;
    _activeRuntimeClient = null;
    setState(() {
      _runtimeStatusState = RuntimeStatusFailed(
        diagnosticCode: diagnosticCode,
        report: report,
        operationKind: failedKind,
        recoveryAction: recoveryAction,
      );
    });
  }

  void _handleRuntimeOperationFailure(
    CoreClientFailure failure, {
    RuntimeOperationKind? operationKind,
  }) {
    if (failure.category != ErrorCategory.cancelled) {
      _failRuntimeOperation(
        failure.code,
        recoveryAction: failure.recoveryAction,
        operationKind: operationKind,
      );
      return;
    }
    if (!mounted) {
      return;
    }
    final report = _runtimeReport();
    _activeRuntimeOperation = null;
    _activeRuntimeClient = null;
    setState(() {
      _runtimeInventoryState = const RuntimeInventoryIdle();
      _runtimeStatusState = RuntimeStatusCancelled(report: report);
    });
  }

  Future<void> _cancelRuntimeOperation() async {
    final operation = _activeRuntimeOperation;
    final client = _activeRuntimeClient;
    if (operation == null || client == null) {
      return;
    }
    try {
      await client.cancelRuntimeOperation(operation);
    } on CoreClientFailure catch (failure) {
      if (_isActiveRuntimeOperation(client, operation)) {
        _handleRuntimeOperationFailure(failure);
      }
    } on Object {
      if (_isActiveRuntimeOperation(client, operation)) {
        _failRuntimeOperation('RUNTIME_OPERATION_CANCEL_FAILED');
      }
    }
  }

  bool _isActiveRuntimeOperation(CoreClient client, CoreOperation operation) {
    return identical(_activeRuntimeClient, client) &&
        _activeRuntimeOperation?.operationId == operation.operationId &&
        _activeRuntimeOperation?.correlationId == operation.correlationId;
  }

  Future<void> _stopRuntimeOperation({required bool cancelRemote}) async {
    final operation = _activeRuntimeOperation;
    final client = _activeRuntimeClient;
    _activeRuntimeOperation = null;
    _activeRuntimeClient = null;
    await _runtimeOperationSubscription?.cancel();
    _runtimeOperationSubscription = null;
    if (cancelRemote && operation != null && client != null) {
      unawaited(_cancelRuntimeQuietly(client, operation));
    }
  }

  Future<void> _cancelRuntimeQuietly(
    CoreClient client,
    CoreOperation operation,
  ) async {
    try {
      await client.cancelRuntimeOperation(operation);
    } on Object {
      // Teardown is already in progress; the supervised sidecar owns cleanup.
    }
  }

  Future<void> _toggleRuntimeModels() async {
    if (_runtimeInventoryState is RuntimeInventoryLoaded) {
      setState(() => _runtimeInventoryState = const RuntimeInventoryIdle());
      return;
    }
    if (_runtimeInventoryState is RuntimeInventoryLoading ||
        _activeRuntimeOperation != null) {
      return;
    }
    final client = widget.coreClient;
    final revision = _runtimeRevision;
    setState(() => _runtimeInventoryState = const RuntimeInventoryLoading());
    try {
      final inventory = await client.listRuntimeModels();
      if (!mounted ||
          !identical(client, widget.coreClient) ||
          revision != _runtimeRevision) {
        return;
      }
      setState(() {
        _runtimeInventoryState = RuntimeInventoryLoaded(inventory: inventory);
      });
    } on CoreClientFailure catch (failure) {
      if (!mounted ||
          !identical(client, widget.coreClient) ||
          revision != _runtimeRevision) {
        return;
      }
      setState(() {
        _runtimeInventoryState = RuntimeInventoryFailed(
          diagnosticCode: failure.code,
          recoveryAction: failure.recoveryAction,
        );
      });
    } on Object {
      if (!mounted ||
          !identical(client, widget.coreClient) ||
          revision != _runtimeRevision) {
        return;
      }
      setState(() {
        _runtimeInventoryState = const RuntimeInventoryFailed(
          diagnosticCode: 'RUNTIME_MODEL_INVENTORY_FAILED',
        );
      });
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

  Future<void> _recoverSetupJob() async {
    await _detachSetupStream();
    if (!mounted) {
      return;
    }
    final client = widget.coreClient;
    final revision = ++_setupRevision;
    final previousJob = _setupJob;
    if (previousJob != null) {
      setState(
        () => _setupState = SetupWorkflowLoading(previousJob: previousJob),
      );
    }
    try {
      final job = await client.recoverSetupJob();
      if (!_isCurrentSetupRequest(client, revision)) {
        return;
      }
      _setupAfterSequence = 0;
      _applySetupSnapshot(job);
      if (job?.state == SetupJobState.active) {
        _observeSetupJob(client, job!.jobId, revision);
      }
    } on CoreClientFailure catch (failure) {
      if (!_isCurrentSetupRequest(client, revision)) {
        return;
      }
      _failSetup(failure.code, previousJob, transportFailure: failure);
    } on Object {
      if (!_isCurrentSetupRequest(client, revision)) {
        return;
      }
      _failSetup('SETUP_RECOVERY_FAILED', previousJob);
    }
  }

  Future<void> _createSetupPlan(RecommendationPlan recommendation) async {
    if (!_canReviewSetup || !mounted) {
      return;
    }
    await _detachSetupStream();
    if (!mounted) {
      return;
    }
    final client = widget.coreClient;
    final revision = ++_setupRevision;
    setState(() => _setupState = const SetupWorkflowPlanning());
    try {
      final job = await client.createSetupPlan(recommendation);
      if (!_isCurrentSetupRequest(client, revision)) {
        return;
      }
      _setupAfterSequence = 0;
      _applySetupSnapshot(job);
    } on CoreClientFailure catch (failure) {
      if (_isCurrentSetupRequest(client, revision)) {
        _failSetup(failure.code, null, transportFailure: failure);
      }
    } on Object {
      if (_isCurrentSetupRequest(client, revision)) {
        _failSetup('SETUP_PLAN_FAILED', null);
      }
    }
  }

  Future<void> _decideSetupApproval(SetupApprovalDecision decision) async {
    final job = _setupJob;
    if (job == null || job.state != SetupJobState.awaitingApproval) {
      return;
    }
    await _runSetupSnapshotRequest(
      job,
      (client) => client.decideSetupApproval(job, decision),
      fallbackDiagnosticCode: 'SETUP_APPROVAL_FAILED',
    );
  }

  Future<void> _startSetupJob() async {
    final job = _setupJob;
    if (job == null || job.state != SetupJobState.approved) {
      return;
    }
    await _runSetupSnapshotRequest(
      job,
      (client) => client.startSetupJob(job),
      fallbackDiagnosticCode: 'SETUP_START_FAILED',
      observeWhenActive: true,
    );
  }

  Future<void> _cancelSetupJob() async {
    final job = _setupJob;
    if (job == null || !_cancellableSetupStates.contains(job.state)) {
      return;
    }
    await _runSetupSnapshotRequest(
      job,
      (client) => client.cancelSetupJob(job.jobId),
      fallbackDiagnosticCode: 'SETUP_CANCEL_FAILED',
      observeWhenActive: true,
    );
  }

  Future<void> _retrySetupJob() async {
    final job = _setupJob;
    if (job == null || !_isRetryableSetupJob(job)) {
      return;
    }
    await _runSetupSnapshotRequest(
      job,
      (client) => client.retrySetupJob(job),
      fallbackDiagnosticCode: 'SETUP_RETRY_FAILED',
      observeWhenActive: true,
    );
  }

  Future<void> _refreshSetupJob() async {
    final job = _setupJob;
    if (job == null) {
      await _recoverSetupJob();
      return;
    }
    await _runSetupSnapshotRequest(
      job,
      (client) => client.setupJobStatus(job.jobId),
      fallbackDiagnosticCode: 'SETUP_STATUS_FAILED',
      observeWhenActive: true,
    );
  }

  Future<void> _runSetupSnapshotRequest(
    SetupJobSnapshot previousJob,
    Future<SetupJobSnapshot> Function(CoreClient client) request, {
    required String fallbackDiagnosticCode,
    bool observeWhenActive = false,
  }) async {
    await _detachSetupStream();
    if (!mounted) {
      return;
    }
    final client = widget.coreClient;
    final revision = ++_setupRevision;
    setState(
      () => _setupState = SetupWorkflowLoading(previousJob: previousJob),
    );
    try {
      final job = await request(client);
      if (!_isCurrentSetupRequest(client, revision)) {
        return;
      }
      _applySetupSnapshot(job);
      if (observeWhenActive && job.state == SetupJobState.active) {
        _observeSetupJob(client, job.jobId, revision);
      }
    } on CoreClientFailure catch (failure) {
      if (_isCurrentSetupRequest(client, revision)) {
        _failSetup(failure.code, previousJob, transportFailure: failure);
      }
    } on Object {
      if (_isCurrentSetupRequest(client, revision)) {
        _failSetup(fallbackDiagnosticCode, previousJob);
      }
    }
  }

  void _observeSetupJob(CoreClient client, SetupJobId jobId, int revision) {
    if (!_isCurrentSetupRequest(client, revision) ||
        _setupSubscription != null) {
      return;
    }
    _setupStreamClient = client;
    _setupStreamJobId = jobId;
    late final StreamSubscription<SetupJobEvent> subscription;
    subscription = client
        .observeSetupJob(jobId, afterSequence: _setupAfterSequence)
        .listen(
          (event) {
            if (_isObservedSetupJob(client, jobId, revision, subscription)) {
              _applySetupEvent(event);
            }
          },
          onError: (Object error) {
            if (_isObservedSetupJob(client, jobId, revision, subscription)) {
              _setupSubscription = null;
              _scheduleSetupStatusRecovery(
                client,
                jobId,
                revision,
                failure: error is CoreClientFailure ? error : null,
              );
            }
          },
          onDone: () {
            if (_isObservedSetupJob(client, jobId, revision, subscription)) {
              _setupSubscription = null;
              if (_setupJob?.state == SetupJobState.active) {
                _scheduleSetupStatusRecovery(client, jobId, revision);
              }
            }
          },
        );
    _setupSubscription = subscription;
  }

  void _applySetupEvent(SetupJobEvent event) {
    if (!mounted || event.sequence <= _setupAfterSequence) {
      return;
    }
    _setupAfterSequence = event.sequence;
    final current = _setupJob;
    if (current == null ||
        event.job.updatedAtUnixMs >= current.updatedAtUnixMs) {
      _applySetupSnapshot(event.job);
    }
  }

  void _scheduleSetupStatusRecovery(
    CoreClient client,
    SetupJobId jobId,
    int revision, {
    CoreClientFailure? failure,
  }) {
    _setupReconnectTimer?.cancel();
    _setupReconnectTimer = Timer(const Duration(milliseconds: 250), () async {
      if (!_isCurrentSetupRequest(client, revision) ||
          _setupStreamJobId != jobId) {
        return;
      }
      final previousJob = _setupJob;
      try {
        final job = await client.setupJobStatus(jobId);
        if (!_isCurrentSetupRequest(client, revision) ||
            _setupStreamJobId != jobId) {
          return;
        }
        _applySetupSnapshot(job);
        if (job.state == SetupJobState.active) {
          _observeSetupJob(client, jobId, revision);
        }
      } on CoreClientFailure catch (statusFailure) {
        if (_isCurrentSetupRequest(client, revision)) {
          _failSetup(
            statusFailure.code,
            previousJob,
            transportFailure: statusFailure,
          );
        }
      } on Object {
        if (_isCurrentSetupRequest(client, revision)) {
          _failSetup(
            failure?.code ?? 'SETUP_EVENT_STREAM_FAILED',
            previousJob,
            transportFailure: failure,
          );
        }
      }
    });
  }

  void _applySetupSnapshot(SetupJobSnapshot? job) {
    if (!mounted) {
      return;
    }
    final previousJobId = _setupJob?.jobId;
    if (job == null) {
      _setupAfterSequence = 0;
    } else if (previousJobId != job.jobId ||
        job.latestEventSequence > _setupAfterSequence) {
      _setupAfterSequence = job.latestEventSequence;
    }
    _setupJob = job;
    setState(() {
      _setupState = job == null
          ? const SetupWorkflowIdle()
          : _mapSetupSnapshot(job);
    });
  }

  SetupWorkflowState _mapSetupSnapshot(SetupJobSnapshot job) {
    return switch (job.state) {
      SetupJobState.awaitingApproval => SetupWorkflowAwaitingApproval(job: job),
      SetupJobState.approved => SetupWorkflowApproved(job: job),
      SetupJobState.active => SetupWorkflowActive(job: job),
      SetupJobState.attentionRequired => SetupWorkflowAttention(job: job),
      SetupJobState.ready => SetupWorkflowReady(job: job),
      SetupJobState.failed => SetupWorkflowFailed(
        diagnosticCode: job.error?.code ?? 'SETUP_JOB_FAILED',
        job: job,
        recoveryAction: job.recoveryAction,
      ),
      SetupJobState.cancelled => SetupWorkflowCancelled(job: job),
      SetupJobState.draftPlan ||
      SetupJobState.unknown => SetupWorkflowUnknown(job: job),
    };
  }

  void _failSetup(
    String diagnosticCode,
    SetupJobSnapshot? previousJob, {
    CoreClientFailure? transportFailure,
  }) {
    if (!mounted) {
      return;
    }
    _setupJob = previousJob;
    setState(() {
      _setupState = SetupWorkflowFailed(
        diagnosticCode: diagnosticCode,
        job: previousJob,
        recoveryAction: previousJob?.recoveryAction,
        transportCategory: transportFailure?.category,
        transportRecoveryAction: transportFailure?.recoveryAction,
        transportRecoveryMessage: transportFailure?.recoveryMessage,
      );
    });
  }

  bool _isCurrentSetupRequest(CoreClient client, int revision) {
    return mounted &&
        identical(client, widget.coreClient) &&
        revision == _setupRevision;
  }

  bool _isObservedSetupJob(
    CoreClient client,
    SetupJobId jobId,
    int revision,
    StreamSubscription<SetupJobEvent> subscription,
  ) {
    return _isCurrentSetupRequest(client, revision) &&
        identical(_setupStreamClient, client) &&
        _setupStreamJobId == jobId &&
        identical(_setupSubscription, subscription);
  }

  Future<void> _detachSetupStream() {
    _setupReconnectTimer?.cancel();
    _setupReconnectTimer = null;
    final subscription = _setupSubscription;
    _setupSubscription = null;
    _setupStreamClient = null;
    _setupStreamJobId = null;
    if (subscription != null) {
      unawaited(subscription.cancel());
    }
    return Future<void>.value();
  }

  bool get _canReviewSetup {
    final jobState = _setupJob?.state;
    return widget.coreClient.supportsTransportCapability(
          TransportCapability.setupWorkflow,
        ) &&
        (_setupState is SetupWorkflowIdle ||
            _setupState is SetupWorkflowReady ||
            jobState == SetupJobState.failed ||
            jobState == SetupJobState.cancelled);
  }

  bool get _canCancelSetup {
    final job = _setupJob;
    return job != null && _cancellableSetupStates.contains(job.state);
  }

  bool get _canRetrySetup {
    final job = _setupJob;
    return job != null && _isRetryableSetupJob(job);
  }

  @override
  Widget build(BuildContext context) {
    return FoundationScreen(
      state: _state,
      hardwareScanState: _hardwareScanState,
      recommendationState: _recommendationState,
      runtimeStatusState: _runtimeStatusState,
      runtimeInventoryState: _runtimeInventoryState,
      setupState: _setupState,
      preferences: _preferences,
      onRetry: _checkConnection,
      onStartHardwareScan: _startHardwareScan,
      onCancelHardwareScan: _cancelHardwareScan,
      onWorkloadChanged: _setWorkload,
      onPriorityChanged: _setPriority,
      onIncludeOptionalLargerChanged: _setIncludeOptionalLarger,
      onGenerateRecommendation: _generateRecommendation,
      onRefreshRuntime:
          widget.coreClient.supportsTransportCapability(
            TransportCapability.runtimeStatus,
          )
          ? _checkRuntimeStatus
          : null,
      onApproveRuntimeReuse: _approveRuntimeReuse,
      onStartRuntimeOperation: _startRuntimeOperation,
      onCancelRuntimeOperation: _activeRuntimeOperation == null
          ? null
          : _cancelRuntimeOperation,
      onToggleRuntimeModels: _toggleRuntimeModels,
      onReviewSetup: _canReviewSetup ? _createSetupPlan : null,
      onApproveSetup: _setupJob?.state == SetupJobState.awaitingApproval
          ? () => _decideSetupApproval(SetupApprovalDecision.approve)
          : null,
      onDenySetup: _setupJob?.state == SetupJobState.awaitingApproval
          ? () => _decideSetupApproval(SetupApprovalDecision.deny)
          : null,
      onStartSetup: _setupJob?.state == SetupJobState.approved
          ? _startSetupJob
          : null,
      onCancelSetup: _canCancelSetup ? _cancelSetupJob : null,
      onRetrySetup: _canRetrySetup ? _retrySetupJob : null,
      onRefreshSetup: _setupState is SetupWorkflowIdle
          ? null
          : _refreshSetupJob,
    );
  }
}

const _retryableSetupStates = {
  SetupJobState.attentionRequired,
  SetupJobState.cancelled,
};

bool _isRetryableSetupJob(SetupJobSnapshot job) {
  return _retryableSetupStates.contains(job.state) ||
      (job.state == SetupJobState.failed &&
          job.recoveryAction == SetupRecoveryAction.retry);
}

const _cancellableSetupStates = {
  SetupJobState.approved,
  SetupJobState.active,
  SetupJobState.attentionRequired,
};
