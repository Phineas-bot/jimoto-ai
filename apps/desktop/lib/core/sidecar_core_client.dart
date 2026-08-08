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
      _hello = hello;
    }
    return session;
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
