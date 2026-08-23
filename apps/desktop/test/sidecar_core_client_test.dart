import 'package:flutter_test/flutter_test.dart';
import 'package:gixgiz_desktop/core/core_client.dart';
import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';
import 'package:gixgiz_desktop/core/sidecar_connection.dart';
import 'package:gixgiz_desktop/core/sidecar_core_client.dart';

import 'support/setup_fixture.dart';

void main() {
  test(
    'maps authoritative Rust health and version into a ready snapshot',
    () async {
      final session = _FakeSession(readiness: ReadinessStatus.ready);
      final client = SidecarCoreClient(
        connector: _FakeConnector.session(session),
      );

      final snapshot = await client.checkConnection();

      expect(snapshot.kind, CoreConnectionKind.ready);
      expect(snapshot.applicationName, 'GixGiz');
      expect(snapshot.coreVersion, '0.1.0');
      expect(snapshot.protocolVersion, 1);
      expect(snapshot.readiness, ReadinessStatus.ready);
    },
  );

  test(
    'maps missing core startup timeout and protocol mismatch distinctly',
    () async {
      final cases = <(SidecarFailure, CoreConnectionIssue)>[
        (
          const SidecarFailure(
            SidecarFailureKind.missingCore,
            'CORE_EXECUTABLE_MISSING',
          ),
          CoreConnectionIssue.missingCore,
        ),
        (
          const SidecarFailure(
            SidecarFailureKind.startupTimedOut,
            'CORE_STARTUP_TIMEOUT',
          ),
          CoreConnectionIssue.startupTimedOut,
        ),
        (
          const SidecarFailure(
            SidecarFailureKind.protocolMismatch,
            'CORE_PROTOCOL_INCOMPATIBLE',
          ),
          CoreConnectionIssue.protocolMismatch,
        ),
      ];

      for (final (failure, expectedIssue) in cases) {
        final client = SidecarCoreClient(
          connector: _FakeConnector.failure(failure),
        );
        final snapshot = await client.checkConnection();
        expect(snapshot.issue, expectedIssue);
        expect(snapshot.diagnosticCode, failure.diagnosticCode);
      }
    },
  );

  test('connection loss is recoverable through a new handshake', () async {
    final session = _FakeSession(
      readiness: ReadinessStatus.ready,
      healthFailure: const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'CORE_CONNECTION_LOST',
      ),
    );
    final client = SidecarCoreClient(
      connector: _FakeConnector.session(session),
    );

    final lost = await client.checkConnection();
    final recovered = await client.checkConnection();

    expect(lost.issue, CoreConnectionIssue.connectionLost);
    expect(recovered.kind, CoreConnectionKind.ready);
    expect(session.handshakeCount, 2);
  });

  test('ordered cancellation event foundation remains typed', () async {
    final session = _FakeSession(readiness: ReadinessStatus.ready);
    final client = SidecarCoreClient(
      connector: _FakeConnector.session(session),
    );
    await client.checkConnection();

    final operation = await client.startFoundationOperation();
    final accepted = await client.cancelFoundationOperation(operation);
    final events = await client.observeFoundationOperation(operation).toList();

    expect(accepted, isTrue);
    expect(events.map((event) => event.sequence), orderedEquals([1, 2]));
    expect(events.last.terminalState, TestOperationTerminalState.cancelled);
    expect(
      events.every((event) => event.correlationId == operation.correlationId),
      isTrue,
    );
  });

  test('hardware scan operation and cancellation remain typed', () async {
    final session = _FakeSession(readiness: ReadinessStatus.ready);
    final client = SidecarCoreClient(
      connector: _FakeConnector.session(session),
    );
    await client.checkConnection();

    final operation = await client.startHardwareScan();
    final accepted = await client.cancelHardwareScan(operation);
    final events = await client.observeHardwareScan(operation).toList();

    expect(accepted, isTrue);
    expect(events.map((event) => event.sequence), orderedEquals([1, 2]));
    expect(events.last.terminalState, HardwareScanTerminalState.cancelled);
  });

  test('capability recommendation remains typed across the session', () async {
    final session = _FakeSession(readiness: ReadinessStatus.ready);
    final client = SidecarCoreClient(
      connector: _FakeConnector.session(session),
    );

    final report = await client.recommendCapability(
      _machineProfile(),
      const UserPreferenceProfile(
        workload: WorkloadTier.generalText,
        priority: PreferencePriority.balanced,
        includeOptionalLarger: true,
      ),
    );

    expect(report.status, CapabilityReportStatus.noPlan);
    expect(report.catalogueVersion, 'test-catalogue');
    expect(
      session.lastHello?.requestedCapabilities,
      contains(TransportCapability.capabilityRecommendation),
    );
  });

  test('runtime status consent lifecycle and inventory remain typed', () async {
    final session = _FakeSession(readiness: ReadinessStatus.ready);
    final client = SidecarCoreClient(
      connector: _FakeConnector.session(session),
    );

    final initial = await client.checkRuntimeStatus();
    final approved = await client.decideRuntimeReuse(
      RuntimeConsentDecision.approveReuse,
    );
    final operation = await client.startRuntimeOperation(
      RuntimeOperationKind.start,
    );
    final cancelled = await client.cancelRuntimeOperation(operation);
    final events = await client.observeRuntimeOperation(operation).toList();
    final inventory = await client.listRuntimeModels();

    expect(initial.state, RuntimeState.installedStopped);
    expect(approved.reuseConsent, RuntimeConsentState.reuseApproved);
    expect(session.lastConsentDecision, RuntimeConsentDecision.approveReuse);
    expect(cancelled, isTrue);
    expect(events.map((event) => event.sequence), orderedEquals([1, 2]));
    expect(events.last.terminalState, RuntimeOperationTerminalState.completed);
    expect(inventory.models.single.displayName, 'Fixture model');
    expect(session.lastRuntimeRequestProviderId, session.runtimeProviderId);
    expect(
      client.supportsTransportCapability(TransportCapability.runtimeStatus),
      isTrue,
    );
    expect(
      session.lastHello?.requestedCapabilities,
      containsAll([
        TransportCapability.runtimeStatus,
        TransportCapability.runtimeConsent,
        TransportCapability.runtimeLifecycle,
        TransportCapability.runtimeModelInventory,
      ]),
    );
  });

  test(
    'chat locality remains typed and requests the negotiated capability',
    () async {
      const conversationId = '00000000-0000-4000-8000-000000000099';
      final session = _FakeSession(readiness: ReadinessStatus.ready);
      final client = SidecarCoreClient(
        connector: _FakeConnector.session(session),
      );

      final status = await client.chatRuntimeStatus(
        conversationId: conversationId,
      );

      expect(status.locality, ChatLocalityStatus.runningLocally);
      expect(status.ready, isTrue);
      expect(
        session.lastChatRuntimeStatusRequest?.conversationId,
        conversationId,
      );
      expect(
        session.lastHello?.requestedCapabilities,
        contains(TransportCapability.localChat),
      );
    },
  );

  test(
    'runtime calls fail locally when capability was not negotiated',
    () async {
      final session = _FakeSession(
        readiness: ReadinessStatus.ready,
        supportedCapabilities: const [TransportCapability.health],
      );
      final client = SidecarCoreClient(
        connector: _FakeConnector.session(session),
      );

      await expectLater(
        client.checkRuntimeStatus(),
        throwsA(
          isA<CoreClientFailure>()
              .having(
                (failure) => failure.category,
                'category',
                ErrorCategory.notSupported,
              )
              .having(
                (failure) => failure.code,
                'code',
                'CORE_CAPABILITY_UNSUPPORTED',
              ),
        ),
      );

      expect(session.runtimeStatusCount, 0);
      expect(
        client.supportsTransportCapability(TransportCapability.runtimeStatus),
        isFalse,
      );
    },
  );

  test(
    'runtime failures preserve safe category and recovery guidance',
    () async {
      final session = _FakeSession(
        readiness: ReadinessStatus.ready,
        runtimeStatusFailure: const SidecarFailure(
          SidecarFailureKind.coreFailure,
          'runtime.endpoint_unavailable',
          safeError: SafeErrorPayload(
            category: ErrorCategory.unavailable,
            code: 'runtime.endpoint_unavailable',
            correlationId: '00000000-0000-4000-8000-000000000010',
            message: 'The runtime endpoint is unavailable.',
            recovery: RecoveryGuidance(
              action: RecoveryAction.restart,
              message: 'Restart the runtime and retry.',
            ),
            requestId: '00000000-0000-4000-8000-000000000011',
          ),
        ),
      );
      final client = SidecarCoreClient(
        connector: _FakeConnector.session(session),
      );

      await expectLater(
        client.checkRuntimeStatus(),
        throwsA(
          isA<CoreClientFailure>()
              .having(
                (failure) => failure.code,
                'code',
                'runtime.endpoint_unavailable',
              )
              .having(
                (failure) => failure.category,
                'category',
                ErrorCategory.unavailable,
              )
              .having(
                (failure) => failure.recoveryAction,
                'recoveryAction',
                RecoveryAction.restart,
              ),
        ),
      );
    },
  );

  test(
    'local runtime failures map to stable categories and recovery',
    () async {
      final cases = <(SidecarFailure, ErrorCategory, RecoveryAction)>[
        (
          const SidecarFailure(
            SidecarFailureKind.connectionLost,
            'RUNTIME_OPERATION_STREAM_TIMEOUT',
          ),
          ErrorCategory.timedOut,
          RecoveryAction.retry,
        ),
        (
          const SidecarFailure(
            SidecarFailureKind.connectionLost,
            'RUNTIME_OPERATION_CONNECTION_LOST',
          ),
          ErrorCategory.unavailable,
          RecoveryAction.retry,
        ),
        (
          const SidecarFailure(
            SidecarFailureKind.invalidResponse,
            'RUNTIME_OPERATION_EVENT_INVALID',
          ),
          ErrorCategory.integrityFailure,
          RecoveryAction.contactSupport,
        ),
      ];

      for (final (failure, category, recoveryAction) in cases) {
        final client = SidecarCoreClient(
          connector: _FakeConnector.session(
            _FakeSession(
              readiness: ReadinessStatus.ready,
              runtimeStatusFailure: failure,
            ),
          ),
        );

        await expectLater(
          client.checkRuntimeStatus(),
          throwsA(
            isA<CoreClientFailure>()
                .having((mapped) => mapped.code, 'code', failure.diagnosticCode)
                .having((mapped) => mapped.category, 'category', category)
                .having(
                  (mapped) => mapped.recoveryAction,
                  'recoveryAction',
                  recoveryAction,
                ),
          ),
        );
      }
    },
  );

  test('runtime provider identity mismatch fails closed', () async {
    final session = _FakeSession(
      readiness: ReadinessStatus.ready,
      reportedRuntimeProviderId: 'unexpected.runtime',
    );
    final client = SidecarCoreClient(
      connector: _FakeConnector.session(session),
    );

    await expectLater(
      client.checkRuntimeStatus(),
      throwsA(
        isA<CoreClientFailure>().having(
          (failure) => failure.code,
          'code',
          'RUNTIME_PROVIDER_ID_MISMATCH',
        ),
      ),
    );
  });

  test('runtime capability without a composed provider fails closed', () async {
    final session = _FakeSession(
      readiness: ReadinessStatus.ready,
      runtimeProviderId: null,
    );
    final client = SidecarCoreClient(
      connector: _FakeConnector.session(session),
    );

    await expectLater(
      client.checkRuntimeStatus(),
      throwsA(
        isA<CoreClientFailure>()
            .having(
              (failure) => failure.code,
              'code',
              'RUNTIME_PROVIDER_ID_MISSING',
            )
            .having(
              (failure) => failure.category,
              'category',
              ErrorCategory.integrityFailure,
            ),
      ),
    );
    expect(session.runtimeStatusCount, 0);
  });

  test('normal shutdown is delegated once to the sidecar session', () async {
    final session = _FakeSession(readiness: ReadinessStatus.ready);
    final client = SidecarCoreClient(
      connector: _FakeConnector.session(session),
    );
    await client.checkConnection();

    await client.shutdown();
    await client.shutdown();

    expect(session.shutdownCount, 1);
  });

  test(
    'setup requests use persisted job identity revision and cursor',
    () async {
      final session = _FakeSession(
        readiness: ReadinessStatus.ready,
        supportedCapabilities: const [
          TransportCapability.health,
          TransportCapability.setupWorkflow,
        ],
      );
      final client = SidecarCoreClient(
        connector: _FakeConnector.session(session),
      );
      await client.checkConnection();

      final planned = await client.createSetupPlan(
        setupRecommendationFixture(),
      );
      expect(planned.state, SetupJobState.awaitingApproval);
      expect(session.lastSetupPlanRequest?.providerId, setupProviderIdFixture);
      expect(
        session.lastSetupPlanRequest?.destination,
        SetupDestinationCategory.providerManaged,
      );

      final recovered = await client.recoverSetupJob();
      expect(recovered, isNotNull);
      final approved = await client.decideSetupApproval(
        recovered!,
        SetupApprovalDecision.approve,
      );
      expect(approved.state, SetupJobState.approved);
      expect(session.lastSetupApprovalRequest?.jobId, setupJobIdFixture);
      expect(session.lastSetupApprovalRequest?.planRevision, 1);

      final started = await client.startSetupJob(approved);
      expect(started.state, SetupJobState.active);
      final event = await client
          .observeSetupJob(started.jobId, afterSequence: 7)
          .single;
      expect(event.sequence, 8);
      expect(session.lastSetupEventsRequest?.limit, 64);
      expect(session.lastSetupEventsRequest?.afterSequence, 7);

      final cancelling = await client.cancelSetupJob(started.jobId);
      expect(cancelling.cancellationRequested, isTrue);
      expect(session.lastSetupCancelRequest?.jobId, started.jobId);

      final retried = await client.retrySetupJob(
        setupJobFixture(state: 'cancelled', stage: 'cancelled'),
      );
      expect(retried.state, SetupJobState.active);
      expect(session.lastSetupRetryRequest?.planRevision, 1);
    },
  );

  test(
    'unknown generated enums retain an explicit forward-compatible value',
    () {
      expect(
        ReadinessStatus.fromJson('future_readiness'),
        ReadinessStatus.unknown,
      );
      expect(
        TransportCapability.fromJson('future_capability'),
        TransportCapability.unknown,
      );
    },
  );
}

class _FakeConnector implements CoreSidecarConnector {
  _FakeConnector.session(this._session) : _failure = null;
  _FakeConnector.failure(this._failure) : _session = null;

  final CoreSidecarSession? _session;
  final SidecarFailure? _failure;

  @override
  Future<CoreSidecarSession> connect() async {
    final failure = _failure;
    if (failure != null) {
      throw failure;
    }
    return _session!;
  }
}

class _FakeSession implements CoreSidecarSession {

  // Runtime installation is exercised by dedicated tests; this stub only needs
  // to satisfy the interface.
  @override
  Future<RuntimeInstallPlanResponse> runtimeInstallPlan(
    RuntimeInstallPlanRequest request,
  ) async => throw UnimplementedError();

  @override
  Future<RuntimeInstallApprovalResponse> runtimeInstallApproval(
    RuntimeInstallApprovalRequest request,
  ) async => throw UnimplementedError();

  @override
  Future<RuntimeInstallStartResponse> runtimeInstallStart(
    RuntimeInstallStartRequest request,
  ) async => throw UnimplementedError();

  @override
  Future<RuntimeInstallStatusResponse> runtimeInstallStatus(
    RuntimeInstallStatusRequest request,
  ) async => throw UnimplementedError();
  _FakeSession({
    required this.readiness,
    this.healthFailure,
    this.runtimeProviderId = 'gixgiz.runtime.test.v1',
    this.reportedRuntimeProviderId,
    this.runtimeStatusFailure,
    this.supportedCapabilities = const [
      TransportCapability.health,
      TransportCapability.runtimeStatus,
      TransportCapability.runtimeConsent,
      TransportCapability.runtimeLifecycle,
      TransportCapability.runtimeModelInventory,
      TransportCapability.localChat,
      TransportCapability.cancellation,
    ],
  });

  static const _instanceId = '00000000-0000-4000-8000-000000000001';
  static const _operationId = '00000000-0000-4000-8000-000000000002';

  final ReadinessStatus readiness;
  final RuntimeProviderId? runtimeProviderId;
  final RuntimeProviderId? reportedRuntimeProviderId;
  final SidecarFailure? runtimeStatusFailure;
  final List<TransportCapability> supportedCapabilities;
  SidecarFailure? healthFailure;
  int handshakeCount = 0;
  int shutdownCount = 0;
  int runtimeStatusCount = 0;
  ClientHello? lastHello;
  RuntimeConsentDecision? lastConsentDecision;
  RuntimeProviderId? lastRuntimeRequestProviderId;
  SetupApprovalRequest? lastSetupApprovalRequest;
  ChatRuntimeStatusRequest? lastChatRuntimeStatusRequest;
  SetupJobCancelRequest? lastSetupCancelRequest;
  SetupJobEventsRequest? lastSetupEventsRequest;
  SetupJobRetryRequest? lastSetupRetryRequest;
  SetupJobStartRequest? lastSetupStartRequest;
  SetupPlanRequest? lastSetupPlanRequest;

  @override
  bool get hasExited => false;

  @override
  Future<CoreHello> handshake(ClientHello hello) async {
    handshakeCount += 1;
    lastHello = hello;
    return CoreHello(
      application: _application,
      selectedProtocol: 1,
      supportedCapabilities: supportedCapabilities,
      runtimeProviderId: runtimeProviderId,
      readiness: _readiness(readiness),
      instanceId: _instanceId,
      correlationId: hello.correlationId,
      requestId: hello.requestId,
    );
  }

  @override
  Future<HealthResponse> health(HealthRequest request) async {
    final failure = healthFailure;
    healthFailure = null;
    if (failure != null) {
      throw failure;
    }
    return HealthResponse(
      status: PlatformStatus(
        application: _application,
        readiness: _readiness(readiness),
      ),
      correlationId: request.correlationId,
      requestId: request.requestId,
    );
  }

  @override
  Future<TestOperationStartResponse> startOperation(
    TestOperationStartRequest request,
  ) async {
    return TestOperationStartResponse(
      operationId: _operationId,
      correlationId: request.correlationId,
      requestId: request.requestId,
    );
  }

  @override
  Stream<TestOperationEvent> operationEvents(
    OperationId operationId,
    CorrelationId correlationId,
    RequestId requestId,
  ) async* {
    yield TestOperationEvent(
      schemaVersion: 1,
      operationId: operationId,
      correlationId: correlationId,
      sequence: 1,
      kind: TestOperationEventKind.started,
      timestampUnixMs: 1,
      message: 'started',
      terminalState: null,
    );
    yield TestOperationEvent(
      schemaVersion: 1,
      operationId: operationId,
      correlationId: correlationId,
      sequence: 2,
      kind: TestOperationEventKind.cancelled,
      timestampUnixMs: 2,
      message: 'cancelled',
      terminalState: TestOperationTerminalState.cancelled,
    );
  }

  @override
  Future<CancelOperationResponse> cancelOperation(
    OperationId operationId,
    CancelOperationRequest request,
  ) async {
    return CancelOperationResponse(
      operationId: operationId,
      accepted: true,
      correlationId: request.correlationId,
      requestId: request.requestId,
    );
  }

  @override
  Future<HardwareScanStartResponse> startHardwareScan(
    HardwareScanStartRequest request,
  ) async {
    return HardwareScanStartResponse(
      operationId: _operationId,
      correlationId: request.correlationId,
      requestId: request.requestId,
    );
  }

  @override
  Stream<HardwareScanEvent> hardwareScanEvents(
    OperationId operationId,
    CorrelationId correlationId,
    RequestId requestId,
  ) async* {
    yield HardwareScanEvent(
      schemaVersion: 1,
      operationId: operationId,
      correlationId: correlationId,
      sequence: 1,
      kind: HardwareScanEventKind.started,
      timestampUnixMs: 1,
      message: null,
      profile: null,
      error: null,
      terminalState: null,
    );
    yield HardwareScanEvent(
      schemaVersion: 1,
      operationId: operationId,
      correlationId: correlationId,
      sequence: 2,
      kind: HardwareScanEventKind.cancelled,
      timestampUnixMs: 2,
      message: null,
      profile: null,
      error: null,
      terminalState: HardwareScanTerminalState.cancelled,
    );
  }

  @override
  Future<CancelOperationResponse> cancelHardwareScan(
    OperationId operationId,
    CancelOperationRequest request,
  ) async {
    return CancelOperationResponse(
      operationId: operationId,
      accepted: true,
      correlationId: request.correlationId,
      requestId: request.requestId,
    );
  }

  @override
  Future<RecommendationResponse> recommend(
    RecommendationRequest request,
  ) async {
    return RecommendationResponse(
      report: _noPlanReport(),
      correlationId: request.correlationId,
      requestId: request.requestId,
    );
  }

  @override
  Future<RuntimeStatusResponse> runtimeStatus(
    RuntimeStatusRequest request,
  ) async {
    runtimeStatusCount += 1;
    lastRuntimeRequestProviderId = request.providerId;
    final failure = runtimeStatusFailure;
    if (failure != null) {
      throw failure;
    }
    return RuntimeStatusResponse(
      report: _runtimeReport(_responseRuntimeProviderId),
      correlationId: request.correlationId,
      requestId: request.requestId,
    );
  }

  @override
  Future<RuntimeConsentResponse> runtimeConsent(
    RuntimeConsentRequest request,
  ) async {
    lastRuntimeRequestProviderId = request.providerId;
    lastConsentDecision = request.decision;
    return RuntimeConsentResponse(
      report: _runtimeReport(
        _responseRuntimeProviderId,
        reuseConsent: request.decision == RuntimeConsentDecision.approveReuse
            ? RuntimeConsentState.reuseApproved
            : RuntimeConsentState.denied,
      ),
      correlationId: request.correlationId,
      requestId: request.requestId,
    );
  }

  @override
  Future<RuntimeModelInventoryResponse> runtimeModels(
    RuntimeModelInventoryRequest request,
  ) async {
    lastRuntimeRequestProviderId = request.providerId;
    return RuntimeModelInventoryResponse(
      inventory: RuntimeModelInventory.fromJson({
        'schema_version': 1,
        'provider_id': _responseRuntimeProviderId,
        'models': [
          {
            'provider_model_id': 'fixture:latest',
            'display_name': 'Fixture model',
            'size_bytes': 1024,
            'mapping': {'status': 'external', 'catalogue_id': null},
          },
        ],
        'truncated': false,
        'collected_at_unix_ms': 1,
      }),
      correlationId: request.correlationId,
      requestId: request.requestId,
    );
  }

  @override
  Future<RuntimeOperationStartResponse> startRuntimeOperation(
    RuntimeOperationStartRequest request,
  ) async {
    lastRuntimeRequestProviderId = request.providerId;
    return RuntimeOperationStartResponse(
      operationId: _operationId,
      correlationId: request.correlationId,
      requestId: request.requestId,
    );
  }

  @override
  Stream<RuntimeOperationEvent> runtimeOperationEvents(
    OperationId operationId,
    CorrelationId correlationId,
    RequestId requestId,
  ) async* {
    yield RuntimeOperationEvent(
      schemaVersion: 1,
      operationId: operationId,
      correlationId: correlationId,
      sequence: 1,
      operationKind: RuntimeOperationKind.start,
      kind: RuntimeOperationEventKind.started,
      timestampUnixMs: 1,
      message: 'started',
      report: null,
      error: null,
      terminalState: null,
    );
    yield RuntimeOperationEvent(
      schemaVersion: 1,
      operationId: operationId,
      correlationId: correlationId,
      sequence: 2,
      operationKind: RuntimeOperationKind.start,
      kind: RuntimeOperationEventKind.completed,
      timestampUnixMs: 2,
      message: 'completed',
      report: _runtimeReport(_responseRuntimeProviderId),
      error: null,
      terminalState: RuntimeOperationTerminalState.completed,
    );
  }

  @override
  Future<CancelOperationResponse> cancelRuntimeOperation(
    OperationId operationId,
    CancelOperationRequest request,
  ) async {
    return CancelOperationResponse(
      operationId: operationId,
      accepted: true,
      correlationId: request.correlationId,
      requestId: request.requestId,
    );
  }

  @override
  Future<SetupPlanResponse> createSetupPlan(SetupPlanRequest request) async {
    lastSetupPlanRequest = request;
    return SetupPlanResponse(
      job: setupJobFixture(),
      correlationId: request.correlationId,
      requestId: request.requestId,
    );
  }

  @override
  Future<SetupJobRecoveryResponse> recoverSetupJob(
    SetupJobRecoveryRequest request,
  ) async {
    return SetupJobRecoveryResponse(
      job: setupJobFixture(),
      correlationId: request.correlationId,
      requestId: request.requestId,
    );
  }

  @override
  Future<SetupApprovalResponse> decideSetupApproval(
    SetupApprovalRequest request,
  ) async {
    lastSetupApprovalRequest = request;
    final job = setupJobFixture(
      state: request.decision == SetupApprovalDecision.approve
          ? 'approved'
          : 'cancelled',
      stage: request.decision == SetupApprovalDecision.approve
          ? 'approved'
          : 'cancelled',
    );
    final plan = job.plan;
    return SetupApprovalResponse(
      job: job,
      approval: SetupApprovalRecord(
        approvedEffects: plan.requiredEffects,
        canonicalModelId: plan.model.artifact.canonicalModelId,
        correlationId: request.correlationId,
        decidedAtUnixMs: 5,
        decision: request.decision,
        destination: plan.model.destination,
        expectedSizeBytes: plan.model.expectedSizeBytes,
        externalRuntimeEffect: true,
        jobId: request.jobId,
        licenceSpdx: plan.model.licenceSpdx,
        planRevision: request.planRevision,
        provenance: plan.model.provenance,
        providerId: plan.model.artifact.providerId,
        providerModelId: plan.model.artifact.providerModelId,
        requestId: request.requestId,
      ),
      correlationId: request.correlationId,
      requestId: request.requestId,
    );
  }

  @override
  Future<SetupJobStartResponse> startSetupJob(
    SetupJobStartRequest request,
  ) async {
    lastSetupStartRequest = request;
    return SetupJobStartResponse(
      job: setupJobFixture(state: 'active', stage: 'preparing'),
      correlationId: request.correlationId,
      requestId: request.requestId,
    );
  }

  @override
  Future<SetupJobStatusResponse> setupJobStatus(
    SetupJobStatusRequest request,
  ) async {
    return SetupJobStatusResponse(
      job: setupJobFixture(state: 'active', stage: 'acquiring'),
      correlationId: request.correlationId,
      requestId: request.requestId,
    );
  }

  @override
  Stream<SetupJobEvent> setupJobEvents(SetupJobEventsRequest request) async* {
    lastSetupEventsRequest = request;
    final job = setupJobFixture(state: 'active', stage: 'acquiring');
    yield setupEventFixture(sequence: request.afterSequence + 1, job: job);
  }

  @override
  Future<SetupJobCancelResponse> cancelSetupJob(
    SetupJobCancelRequest request,
  ) async {
    lastSetupCancelRequest = request;
    return SetupJobCancelResponse(
      accepted: true,
      job: setupJobFixture(
        state: 'active',
        stage: 'acquiring',
        cancellationRequested: true,
      ),
      correlationId: request.correlationId,
      requestId: request.requestId,
    );
  }

  @override
  Future<SetupJobRetryResponse> retrySetupJob(
    SetupJobRetryRequest request,
  ) async {
    lastSetupRetryRequest = request;
    return SetupJobRetryResponse(
      job: setupJobFixture(state: 'active', stage: 'preparing'),
      correlationId: request.correlationId,
      requestId: request.requestId,
    );
  }

  @override
  Future<ChatRuntimeStatusResponse> chatRuntimeStatus(
    ChatRuntimeStatusRequest request,
  ) async {
    lastChatRuntimeStatusRequest = request;
    return ChatRuntimeStatusResponse(
      status: const ChatRuntimeStatus(
        blockedBy: null,
        locality: ChatLocalityStatus.runningLocally,
        model: ChatModelIdentity(
          canonicalModelId: 'fixture-model',
          displayName: 'Fixture model',
          family: 'fixture',
        ),
        providerId: 'gixgiz.runtime.test.v1',
        ready: true,
        recoveryAction: null,
        runtimeDisplayName: 'Fixture runtime',
        schemaVersion: 1,
      ),
      correlationId: request.correlationId,
      requestId: request.requestId,
    );
  }

  @override
  Future<CreateConversationResponse> createConversation(
    CreateConversationRequest request,
  ) => throw UnimplementedError();

  @override
  Future<ListConversationsResponse> listConversations(
    ListConversationsRequest request,
  ) => throw UnimplementedError();

  @override
  Future<GetConversationResponse> getConversation(
    GetConversationRequest request,
  ) => throw UnimplementedError();

  @override
  Future<RenameConversationResponse> renameConversation(
    RenameConversationRequest request,
  ) => throw UnimplementedError();

  @override
  Future<DeleteConversationResponse> deleteConversation(
    DeleteConversationRequest request,
  ) => throw UnimplementedError();

  @override
  Future<SendMessageResponse> sendChatMessage(SendMessageRequest request) =>
      throw UnimplementedError();

  @override
  Stream<ChatGenerationEvent> chatGenerationEvents(
    ChatGenerationEventsRequest request,
  ) => throw UnimplementedError();

  @override
  Future<CancelGenerationResponse> cancelChatGeneration(
    CancelGenerationRequest request,
  ) => throw UnimplementedError();

  @override
  Future<void> shutdown(ShutdownRequest request) async {
    shutdownCount += 1;
  }

  RuntimeProviderId get _responseRuntimeProviderId =>
      reportedRuntimeProviderId ??
      runtimeProviderId ??
      'gixgiz.runtime.missing-test-provider.v1';

  static const _application = ApplicationInfo(
    name: 'GixGiz',
    applicationId: 'ai.gixgiz.desktop',
    version: '0.1.0',
    protocolVersion: 1,
    schemaVersion: 1,
  );

  static ReadinessReport _readiness(ReadinessStatus status) {
    return ReadinessReport(
      status: status,
      summary: 'Authoritative Rust readiness.',
      services: const [],
    );
  }
}

RuntimeHealthReport _runtimeReport(
  RuntimeProviderId providerId, {
  RuntimeConsentState reuseConsent = RuntimeConsentState.notRequested,
}) {
  return RuntimeHealthReport.fromJson({
    'schema_version': 1,
    'provider_id': providerId,
    'display_name': 'Ollama',
    'state': 'installed_stopped',
    'ownership': 'external',
    'reuse_consent': reuseConsent.wireValue,
    'management_consent': 'not_requested',
    'endpoint_safety': 'loopback_verified',
    'version': null,
    'capabilities': <Object?>[],
    'reasons': <Object?>[],
    'warnings': <Object?>[],
  });
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
    'scan_id': '00000000-0000-4000-8000-000000000003',
    'correlation_id': '00000000-0000-4000-8000-000000000004',
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

CapabilityReport _noPlanReport() {
  return CapabilityReport.fromJson({
    'schema_version': 1,
    'catalogue_version': 'test-catalogue',
    'rule_set_version': 'test-rules',
    'machine_profile_schema_version': 1,
    'generated_from_scan_unix_ms': 1,
    'preferences': {
      'workload': 'general_text',
      'priority': 'balanced',
      'include_optional_larger': true,
    },
    'status': 'no_plan',
    'recommended_plan': null,
    'fallback_plan': null,
    'optional_larger_plan': null,
    'no_plan': {
      'confidence': 'high',
      'reasons': <Object?>[],
      'warnings': <Object?>[],
    },
    'confidence': 'high',
    'reasons': <Object?>[],
    'warnings': <Object?>[],
  });
}
