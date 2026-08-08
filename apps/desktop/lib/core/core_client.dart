import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';

enum CoreConnectionKind { ready, degraded, unavailable, failed, cancelled }

enum CoreConnectionIssue {
  none,
  missingCore,
  startupTimedOut,
  protocolMismatch,
  authenticationFailed,
  connectionLost,
  coreUnavailable,
  cancelled,
  internalFailure,
}

class CoreConnectionSnapshot {
  const CoreConnectionSnapshot({
    required this.kind,
    this.issue = CoreConnectionIssue.none,
    this.diagnosticCode,
    this.applicationName,
    this.coreVersion,
    this.protocolVersion,
    this.readiness,
    this.readinessSummary,
  });

  final CoreConnectionKind kind;
  final CoreConnectionIssue issue;
  final String? diagnosticCode;
  final String? applicationName;
  final String? coreVersion;
  final int? protocolVersion;
  final ReadinessStatus? readiness;
  final String? readinessSummary;
}

class CoreOperation {
  const CoreOperation({required this.operationId, required this.correlationId});

  final OperationId operationId;
  final CorrelationId correlationId;
}

abstract class CoreClient {
  const CoreClient();

  Future<CoreConnectionSnapshot> checkConnection();

  Future<CoreOperation> startFoundationOperation() {
    return Future.error(
      UnsupportedError(
        'Foundation operations are not supported by this client.',
      ),
    );
  }

  Stream<TestOperationEvent> observeFoundationOperation(
    CoreOperation operation,
  ) {
    return Stream.error(
      UnsupportedError(
        'Foundation event streams are not supported by this client.',
      ),
    );
  }

  Future<bool> cancelFoundationOperation(CoreOperation operation) {
    return Future.error(
      UnsupportedError(
        'Foundation cancellation is not supported by this client.',
      ),
    );
  }

  Future<CoreOperation> startHardwareScan() {
    return Future.error(
      UnsupportedError('Hardware scans are not supported by this client.'),
    );
  }

  Stream<HardwareScanEvent> observeHardwareScan(CoreOperation operation) {
    return Stream.error(
      UnsupportedError(
        'Hardware scan event streams are not supported by this client.',
      ),
    );
  }

  Future<bool> cancelHardwareScan(CoreOperation operation) {
    return Future.error(
      UnsupportedError('Hardware scan cancellation is not supported.'),
    );
  }

  Future<void> shutdown() async {}
}

class DisconnectedCoreClient extends CoreClient {
  const DisconnectedCoreClient();

  @override
  Future<CoreConnectionSnapshot> checkConnection() async {
    return const CoreConnectionSnapshot(
      kind: CoreConnectionKind.unavailable,
      issue: CoreConnectionIssue.coreUnavailable,
      diagnosticCode: 'CORE_NOT_CONNECTED',
    );
  }
}
