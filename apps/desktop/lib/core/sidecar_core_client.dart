import 'package:gixgiz_desktop/core/core_client.dart';
import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';
import 'package:gixgiz_desktop/core/sidecar_connection.dart';

class SidecarCoreClient extends CoreClient {
  SidecarCoreClient({
    CoreSidecarConnector? connector,
    this.clientName = 'GixGiz desktop',
    this.clientVersion = '0.1.0',
  }) : _connector = connector ?? PipeSidecarConnector();

  final CoreSidecarConnector _connector;
  final String clientName;
  final String clientVersion;
  CoreSidecarSession? _session;
  CoreHello? _hello;

  @override
  bool supportsTransportCapability(TransportCapability capability) {
    return _hello?.supportedCapabilities.contains(capability) ?? false;
  }

  @override
  Future<CoreConnectionSnapshot> checkConnection() async {
    try {
      final session = await _connectedSession();
      final health = await session.health(
        HealthRequest(
          correlationId: newCorrelationId(),
          requestId: newRequestId(),
        ),
      );
      return _snapshot(health);
    } on SidecarFailure catch (failure) {
      if (failure.kind == SidecarFailureKind.connectionLost) {
        _hello = null;
      }
      return _failureSnapshot(failure);
    } on Object {
      return const CoreConnectionSnapshot(
        kind: CoreConnectionKind.failed,
        issue: CoreConnectionIssue.internalFailure,
        diagnosticCode: 'CORE_CLIENT_INTERNAL',
      );
    }
  }

  @override
  Future<CoreOperation> startFoundationOperation() async {
    final session = await _connectedSession();
    final correlationId = newCorrelationId();
    final response = await session.startOperation(
      TestOperationStartRequest(
        correlationId: correlationId,
        requestId: newRequestId(),
      ),
    );
    return CoreOperation(
      operationId: response.operationId,
      correlationId: response.correlationId,
    );
  }

  @override
  Stream<TestOperationEvent> observeFoundationOperation(
    CoreOperation operation,
  ) async* {
    final session = await _connectedSession();
    yield* session.operationEvents(
      operation.operationId,
      operation.correlationId,
      newRequestId(),
    );
  }

  @override
  Future<bool> cancelFoundationOperation(CoreOperation operation) async {
    final session = await _connectedSession();
    final response = await session.cancelOperation(
      operation.operationId,
      CancelOperationRequest(
        correlationId: operation.correlationId,
        requestId: newRequestId(),
      ),
    );
    return response.accepted;
  }

  @override
  Future<CoreOperation> startHardwareScan() async {
    final session = await _connectedSession();
    final correlationId = newCorrelationId();
    final response = await session.startHardwareScan(
      HardwareScanStartRequest(
        correlationId: correlationId,
        requestId: newRequestId(),
      ),
    );
    return CoreOperation(
      operationId: response.operationId,
      correlationId: response.correlationId,
    );
  }

  @override
  Stream<HardwareScanEvent> observeHardwareScan(
    CoreOperation operation,
  ) async* {
    final session = await _connectedSession();
    yield* session.hardwareScanEvents(
      operation.operationId,
      operation.correlationId,
      newRequestId(),
    );
  }

  @override
  Future<bool> cancelHardwareScan(CoreOperation operation) async {
    final session = await _connectedSession();
    final response = await session.cancelHardwareScan(
      operation.operationId,
      CancelOperationRequest(
        correlationId: operation.correlationId,
        requestId: newRequestId(),
      ),
    );
    return response.accepted;
  }

  @override
  Future<CapabilityReport> recommendCapability(
    MachineProfile profile,
    UserPreferenceProfile preferences,
  ) async {
    final session = await _connectedSession();
    final response = await session.recommend(
      RecommendationRequest(
        machineProfile: profile,
        preferences: preferences,
        correlationId: newCorrelationId(),
        requestId: newRequestId(),
      ),
    );
    return response.report;
  }

  @override
  Future<RuntimeHealthReport> checkRuntimeStatus() async {
    return _runtimeRequest(TransportCapability.runtimeStatus, (session) async {
      final response = await session.runtimeStatus(
        RuntimeStatusRequest(
          providerId: _runtimeProviderId(),
          correlationId: newCorrelationId(),
          requestId: newRequestId(),
        ),
      );
      _verifyRuntimeProvider(response.report.providerId);
      return response.report;
    });
  }

  @override
  Future<RuntimeHealthReport> decideRuntimeReuse(
    RuntimeConsentDecision decision,
  ) async {
    return _runtimeRequest(TransportCapability.runtimeConsent, (session) async {
      final response = await session.runtimeConsent(
        RuntimeConsentRequest(
          providerId: _runtimeProviderId(),
          decision: decision,
          correlationId: newCorrelationId(),
          requestId: newRequestId(),
        ),
      );
      _verifyRuntimeProvider(response.report.providerId);
      return response.report;
    });
  }

  @override
  Future<CoreOperation> startRuntimeOperation(RuntimeOperationKind kind) async {
    return _runtimeRequest(TransportCapability.runtimeLifecycle, (
      session,
    ) async {
      final correlationId = newCorrelationId();
      final response = await session.startRuntimeOperation(
        RuntimeOperationStartRequest(
          providerId: _runtimeProviderId(),
          kind: kind,
          correlationId: correlationId,
          requestId: newRequestId(),
        ),
      );
      return CoreOperation(
        operationId: response.operationId,
        correlationId: response.correlationId,
      );
    });
  }

  @override
  Stream<RuntimeOperationEvent> observeRuntimeOperation(
    CoreOperation operation,
  ) async* {
    try {
      final session = await _connectedSession();
      _requireCapability(TransportCapability.runtimeLifecycle);
      await for (final event in session.runtimeOperationEvents(
        operation.operationId,
        operation.correlationId,
        newRequestId(),
      )) {
        final providerId = event.report?.providerId;
        if (providerId != null) {
          _verifyRuntimeProvider(providerId);
        }
        yield event;
      }
    } on SidecarFailure catch (failure) {
      throw _coreFailure(failure);
    }
  }

  @override
  Future<bool> cancelRuntimeOperation(CoreOperation operation) async {
    return _runtimeRequest(TransportCapability.runtimeLifecycle, (
      session,
    ) async {
      _requireCapability(TransportCapability.cancellation);
      final response = await session.cancelRuntimeOperation(
        operation.operationId,
        CancelOperationRequest(
          correlationId: operation.correlationId,
          requestId: newRequestId(),
        ),
      );
      return response.accepted;
    });
  }

  @override
  Future<RuntimeModelInventory> listRuntimeModels() async {
    return _runtimeRequest(TransportCapability.runtimeModelInventory, (
      session,
    ) async {
      final response = await session.runtimeModels(
        RuntimeModelInventoryRequest(
          providerId: _runtimeProviderId(),
          limit: 32,
          correlationId: newCorrelationId(),
          requestId: newRequestId(),
        ),
      );
      _verifyRuntimeProvider(response.inventory.providerId);
      return response.inventory;
    });
  }

  @override
  Future<SetupJobSnapshot> createSetupPlan(
    RecommendationPlan recommendation, {
    SetupDestinationCategory destination =
        SetupDestinationCategory.providerManaged,
  }) async {
    return _setupRequest((session) async {
      final response = await session.createSetupPlan(
        SetupPlanRequest(
          recommendation: recommendation,
          providerId: _runtimeProviderId(),
          destination: destination,
          correlationId: newCorrelationId(),
          requestId: newRequestId(),
        ),
      );
      _verifySetupJob(response.job);
      return response.job;
    });
  }

  @override
  Future<SetupJobSnapshot?> recoverSetupJob() async {
    return _setupRequest((session) async {
      final response = await session.recoverSetupJob(
        SetupJobRecoveryRequest(
          correlationId: newCorrelationId(),
          requestId: newRequestId(),
        ),
      );
      final job = response.job;
      if (job != null) {
        _verifySetupJob(job);
      }
      return job;
    });
  }

  @override
  Future<SetupJobSnapshot> decideSetupApproval(
    SetupJobSnapshot job,
    SetupApprovalDecision decision,
  ) async {
    return _setupRequest((session) async {
      _verifySetupJob(job);
      final response = await session.decideSetupApproval(
        SetupApprovalRequest(
          jobId: job.jobId,
          planRevision: job.plan.revision,
          decision: decision,
          correlationId: newCorrelationId(),
          requestId: newRequestId(),
        ),
      );
      _verifySetupJob(response.job, expectedJobId: job.jobId);
      return response.job;
    });
  }

  @override
  Future<SetupJobSnapshot> startSetupJob(SetupJobSnapshot job) async {
    return _setupRequest((session) async {
      _verifySetupJob(job);
      final response = await session.startSetupJob(
        SetupJobStartRequest(
          jobId: job.jobId,
          planRevision: job.plan.revision,
          correlationId: newCorrelationId(),
          requestId: newRequestId(),
        ),
      );
      _verifySetupJob(response.job, expectedJobId: job.jobId);
      return response.job;
    });
  }

  @override
  Future<SetupJobSnapshot> setupJobStatus(SetupJobId jobId) async {
    return _setupRequest((session) async {
      final response = await session.setupJobStatus(
        SetupJobStatusRequest(
          jobId: jobId,
          correlationId: newCorrelationId(),
          requestId: newRequestId(),
        ),
      );
      _verifySetupJob(response.job, expectedJobId: jobId);
      return response.job;
    });
  }

  @override
  Stream<SetupJobEvent> observeSetupJob(
    SetupJobId jobId, {
    required int afterSequence,
  }) async* {
    try {
      final session = await _connectedSession();
      _requireCapability(TransportCapability.setupWorkflow);
      await for (final event in session.setupJobEvents(
        SetupJobEventsRequest(
          jobId: jobId,
          afterSequence: afterSequence,
          limit: 64,
          correlationId: newCorrelationId(),
          requestId: newRequestId(),
        ),
      )) {
        _verifySetupJob(event.job, expectedJobId: jobId);
        yield event;
      }
    } on SidecarFailure catch (failure) {
      throw _coreFailure(failure);
    }
  }

  @override
  Future<SetupJobSnapshot> cancelSetupJob(SetupJobId jobId) async {
    return _setupRequest((session) async {
      final response = await session.cancelSetupJob(
        SetupJobCancelRequest(
          jobId: jobId,
          correlationId: newCorrelationId(),
          requestId: newRequestId(),
        ),
      );
      _verifySetupJob(response.job, expectedJobId: jobId);
      return response.job;
    });
  }

  @override
  Future<SetupJobSnapshot> retrySetupJob(SetupJobSnapshot job) async {
    return _setupRequest((session) async {
      _verifySetupJob(job);
      final response = await session.retrySetupJob(
        SetupJobRetryRequest(
          jobId: job.jobId,
          planRevision: job.plan.revision,
          correlationId: newCorrelationId(),
          requestId: newRequestId(),
        ),
      );
      _verifySetupJob(response.job, expectedJobId: job.jobId);
      return response.job;
    });
  }

  Future<T> _setupRequest<T>(
    Future<T> Function(CoreSidecarSession session) request,
  ) async {
    try {
      final session = await _connectedSession();
      _requireCapability(TransportCapability.setupWorkflow);
      return await request(session);
    } on SidecarFailure catch (failure) {
      throw _coreFailure(failure);
    }
  }

  Future<T> _runtimeRequest<T>(
    TransportCapability capability,
    Future<T> Function(CoreSidecarSession session) request,
  ) async {
    try {
      final session = await _connectedSession();
      _requireCapability(capability);
      return await request(session);
    } on SidecarFailure catch (failure) {
      throw _coreFailure(failure);
    }
  }

  void _requireCapability(TransportCapability capability) {
    if (!supportsTransportCapability(capability)) {
      throw const CoreClientFailure(
        code: 'CORE_CAPABILITY_UNSUPPORTED',
        category: ErrorCategory.notSupported,
        recoveryAction: RecoveryAction.checkPrerequisites,
      );
    }
  }

  CoreClientFailure _coreFailure(SidecarFailure failure) {
    final safeError = failure.safeError;
    if (safeError != null) {
      return CoreClientFailure(
        code: failure.diagnosticCode,
        category: safeError.category,
        recoveryAction: safeError.recovery.action,
        recoveryMessage: safeError.recovery.message,
      );
    }
    final timedOut = failure.diagnosticCode.endsWith('TIMEOUT');
    final category = switch (failure.kind) {
      SidecarFailureKind.missingCore ||
      SidecarFailureKind.startupFailed => ErrorCategory.unavailable,
      SidecarFailureKind.startupTimedOut => ErrorCategory.timedOut,
      SidecarFailureKind.protocolMismatch => ErrorCategory.incompatibleVersion,
      SidecarFailureKind.authenticationFailed => ErrorCategory.permissionDenied,
      SidecarFailureKind.connectionLost when timedOut => ErrorCategory.timedOut,
      SidecarFailureKind.connectionLost => ErrorCategory.unavailable,
      SidecarFailureKind.cancelled => ErrorCategory.cancelled,
      SidecarFailureKind.invalidResponse => ErrorCategory.integrityFailure,
      SidecarFailureKind.coreFailure => ErrorCategory.internal,
    };
    final recoveryAction = switch (category) {
      ErrorCategory.cancelled => RecoveryAction.noAction,
      ErrorCategory.unavailable ||
      ErrorCategory.timedOut => RecoveryAction.retry,
      ErrorCategory.incompatibleVersion => RecoveryAction.checkPrerequisites,
      ErrorCategory.permissionDenied ||
      ErrorCategory.integrityFailure ||
      ErrorCategory.internal => RecoveryAction.contactSupport,
      _ => RecoveryAction.retry,
    };
    return CoreClientFailure(
      code: failure.diagnosticCode,
      category: category,
      recoveryAction: recoveryAction,
    );
  }

  void _verifyRuntimeProvider(RuntimeProviderId providerId) {
    if (providerId != _runtimeProviderId()) {
      throw const SidecarFailure(
        SidecarFailureKind.invalidResponse,
        'RUNTIME_PROVIDER_ID_MISMATCH',
      );
    }
  }

  void _verifySetupJob(SetupJobSnapshot job, {SetupJobId? expectedJobId}) {
    if (job.schemaVersion != coreSchemaVersion ||
        job.plan.schemaVersion != coreSchemaVersion ||
        !isValidTransportUuid(job.jobId) ||
        job.plan.jobId != job.jobId ||
        (expectedJobId != null && job.jobId != expectedJobId) ||
        job.plan.model.artifact.providerId != _runtimeProviderId()) {
      throw const SidecarFailure(
        SidecarFailureKind.invalidResponse,
        'SETUP_JOB_RESPONSE_INVALID',
      );
    }
  }

  @override
  Future<void> shutdown() async {
    final session = _session;
    _session = null;
    _hello = null;
    if (session == null) {
      return;
    }
    await session.shutdown(
      ShutdownRequest(
        correlationId: newCorrelationId(),
        requestId: newRequestId(),
      ),
    );
  }

  Future<CoreSidecarSession> _connectedSession() async {
    var session = _session;
    if (session == null || session.hasExited) {
      session = await _connector.connect();
      _session = session;
      _hello = null;
    }
    if (_hello == null) {
      final correlationId = newCorrelationId();
      final hello = await session.handshake(
        ClientHello(
          clientName: clientName,
          clientVersion: clientVersion,
          protocolMin: coreProtocolMin,
          protocolMax: coreProtocolMax,
          requestedCapabilities: const [
            TransportCapability.health,
            TransportCapability.testOperationEvents,
            TransportCapability.hardwareScan,
            TransportCapability.capabilityRecommendation,
            TransportCapability.runtimeStatus,
            TransportCapability.runtimeConsent,
            TransportCapability.runtimeLifecycle,
            TransportCapability.runtimeModelInventory,
            TransportCapability.setupWorkflow,
            TransportCapability.cancellation,
            TransportCapability.shutdown,
          ],
          correlationId: correlationId,
          requestId: newRequestId(),
        ),
      );
      if (hello.selectedProtocol < coreProtocolMin ||
          hello.selectedProtocol > coreProtocolMax) {
        throw const SidecarFailure(
          SidecarFailureKind.protocolMismatch,
          'CORE_PROTOCOL_INCOMPATIBLE',
        );
      }
      if (_hasRuntimeCapability(hello.supportedCapabilities) &&
          (hello.runtimeProviderId == null ||
              hello.runtimeProviderId!.isEmpty)) {
        throw const SidecarFailure(
          SidecarFailureKind.invalidResponse,
          'RUNTIME_PROVIDER_ID_MISSING',
        );
      }
      _hello = hello;
    }
    return session;
  }

  RuntimeProviderId _runtimeProviderId() {
    final providerId = _hello?.runtimeProviderId;
    if (providerId == null || providerId.isEmpty) {
      throw const SidecarFailure(
        SidecarFailureKind.invalidResponse,
        'RUNTIME_PROVIDER_ID_MISSING',
      );
    }
    return providerId;
  }

  bool _hasRuntimeCapability(List<TransportCapability> capabilities) {
    return capabilities.any(
      (capability) =>
          capability == TransportCapability.runtimeStatus ||
          capability == TransportCapability.runtimeConsent ||
          capability == TransportCapability.runtimeLifecycle ||
          capability == TransportCapability.runtimeModelInventory ||
          capability == TransportCapability.setupWorkflow,
    );
  }

  CoreConnectionSnapshot _snapshot(HealthResponse response) {
    final status = response.status.readiness.status;
    final kind = switch (status) {
      ReadinessStatus.ready => CoreConnectionKind.ready,
      ReadinessStatus.degraded => CoreConnectionKind.degraded,
      ReadinessStatus.unavailable ||
      ReadinessStatus.unknown => CoreConnectionKind.unavailable,
      ReadinessStatus.failed => CoreConnectionKind.failed,
    };
    final issue = switch (status) {
      ReadinessStatus.unavailable ||
      ReadinessStatus.unknown => CoreConnectionIssue.coreUnavailable,
      ReadinessStatus.failed => CoreConnectionIssue.internalFailure,
      _ => CoreConnectionIssue.none,
    };
    return CoreConnectionSnapshot(
      kind: kind,
      issue: issue,
      diagnosticCode: switch (status) {
        ReadinessStatus.unavailable => 'CORE_UNAVAILABLE',
        ReadinessStatus.failed => 'CORE_FAILED',
        ReadinessStatus.unknown => 'CORE_READINESS_UNKNOWN',
        _ => null,
      },
      applicationName: response.status.application.name,
      coreVersion: response.status.application.version,
      protocolVersion: _hello?.selectedProtocol,
      readiness: status,
      readinessSummary: response.status.readiness.summary,
    );
  }

  CoreConnectionSnapshot _failureSnapshot(SidecarFailure failure) {
    final issue = switch (failure.kind) {
      SidecarFailureKind.missingCore => CoreConnectionIssue.missingCore,
      SidecarFailureKind.startupTimedOut => CoreConnectionIssue.startupTimedOut,
      SidecarFailureKind.protocolMismatch =>
        CoreConnectionIssue.protocolMismatch,
      SidecarFailureKind.authenticationFailed =>
        CoreConnectionIssue.authenticationFailed,
      SidecarFailureKind.connectionLost => CoreConnectionIssue.connectionLost,
      SidecarFailureKind.cancelled => CoreConnectionIssue.cancelled,
      _ => CoreConnectionIssue.internalFailure,
    };
    final kind = switch (failure.kind) {
      SidecarFailureKind.missingCore ||
      SidecarFailureKind.startupTimedOut ||
      SidecarFailureKind.connectionLost => CoreConnectionKind.unavailable,
      SidecarFailureKind.cancelled => CoreConnectionKind.cancelled,
      _ => CoreConnectionKind.failed,
    };
    return CoreConnectionSnapshot(
      kind: kind,
      issue: issue,
      diagnosticCode: failure.diagnosticCode,
    );
  }
}
