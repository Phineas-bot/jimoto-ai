import 'package:flutter_test/flutter_test.dart';
import 'package:gixgiz_desktop/core/core_client.dart';
import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';
import 'package:gixgiz_desktop/core/sidecar_connection.dart';
import 'package:gixgiz_desktop/core/sidecar_core_client.dart';

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
  _FakeSession({required this.readiness, this.healthFailure});

  static const _instanceId = '00000000-0000-4000-8000-000000000001';
  static const _operationId = '00000000-0000-4000-8000-000000000002';

  final ReadinessStatus readiness;
  SidecarFailure? healthFailure;
  int handshakeCount = 0;
  int shutdownCount = 0;

  @override
  bool get hasExited => false;

  @override
  Future<CoreHello> handshake(ClientHello hello) async {
    handshakeCount += 1;
    return CoreHello(
      application: _application,
      selectedProtocol: 1,
      supportedCapabilities: const [TransportCapability.health],
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
  Future<void> shutdown(ShutdownRequest request) async {
    shutdownCount += 1;
  }

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
