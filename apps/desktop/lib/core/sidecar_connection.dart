import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math';

import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';

const int coreProtocolMin = 1;
const int coreProtocolMax = 1;
const int coreSchemaVersion = 1;

enum SidecarFailureKind {
  missingCore,
  startupTimedOut,
  startupFailed,
  protocolMismatch,
  authenticationFailed,
  connectionLost,
  cancelled,
  invalidResponse,
  coreFailure,
}

class SidecarFailure implements Exception {
  const SidecarFailure(this.kind, this.diagnosticCode, {this.safeError});

  final SidecarFailureKind kind;
  final String diagnosticCode;
  final SafeErrorPayload? safeError;

  @override
  String toString() => 'SidecarFailure($kind, $diagnosticCode)';
}

abstract interface class CoreSidecarConnector {
  Future<CoreSidecarSession> connect();
}

abstract interface class CoreSidecarSession {
  bool get hasExited;

  Future<CoreHello> handshake(ClientHello hello);

  Future<HealthResponse> health(HealthRequest request);

  Future<TestOperationStartResponse> startOperation(
    TestOperationStartRequest request,
  );

  Stream<TestOperationEvent> operationEvents(
    OperationId operationId,
    CorrelationId correlationId,
    RequestId requestId,
  );

  Future<CancelOperationResponse> cancelOperation(
    OperationId operationId,
    CancelOperationRequest request,
  );

  Future<HardwareScanStartResponse> startHardwareScan(
    HardwareScanStartRequest request,
  );

  Stream<HardwareScanEvent> hardwareScanEvents(
    OperationId operationId,
    CorrelationId correlationId,
    RequestId requestId,
  );

  Future<CancelOperationResponse> cancelHardwareScan(
    OperationId operationId,
    CancelOperationRequest request,
  );

  Future<RecommendationResponse> recommend(RecommendationRequest request);

  Future<RuntimeStatusResponse> runtimeStatus(RuntimeStatusRequest request);

  Future<RuntimeConsentResponse> runtimeConsent(RuntimeConsentRequest request);

  Future<RuntimeModelInventoryResponse> runtimeModels(
    RuntimeModelInventoryRequest request,
  );

  Future<RuntimeOperationStartResponse> startRuntimeOperation(
    RuntimeOperationStartRequest request,
  );

  Stream<RuntimeOperationEvent> runtimeOperationEvents(
    OperationId operationId,
    CorrelationId correlationId,
    RequestId requestId,
  );

  Future<CancelOperationResponse> cancelRuntimeOperation(
    OperationId operationId,
    CancelOperationRequest request,
  );

  Future<void> shutdown(ShutdownRequest request);
}

typedef CorePathResolver = String Function();

// The host allows 30 seconds for lifecycle work and 3 seconds for terminal status.
const _defaultRuntimeEventInactivityTimeout = Duration(seconds: 40);

class PipeSidecarConnector implements CoreSidecarConnector {
  PipeSidecarConnector({
    CorePathResolver? corePathResolver,
    this.startupTimeout = const Duration(seconds: 5),
    this.requestTimeout = const Duration(seconds: 5),
    this.runtimeEventInactivityTimeout = _defaultRuntimeEventInactivityTimeout,
    this.shutdownTimeout = const Duration(seconds: 3),
  }) : _corePathResolver = corePathResolver ?? bundledCorePath;

  final CorePathResolver _corePathResolver;
  final Duration startupTimeout;
  final Duration requestTimeout;
  final Duration runtimeEventInactivityTimeout;
  final Duration shutdownTimeout;

  @override
  Future<CoreSidecarSession> connect() async {
    final executable = _corePathResolver();
    final token = _secureHex(32);
    Process process;
    try {
      process = await Process.start(
        executable,
        const <String>[],
        mode: ProcessStartMode.normal,
        runInShell: false,
      );
    } on ProcessException {
      throw const SidecarFailure(
        SidecarFailureKind.missingCore,
        'CORE_EXECUTABLE_MISSING',
      );
    }

    var exited = false;
    unawaited(process.exitCode.then((_) => exited = true));
    process.stderr.listen((_) {}, onError: (_) {});
    final stdoutLines = StreamIterator<String>(
      process.stdout.transform(utf8.decoder).transform(const LineSplitter()),
    );
    final request = BootstrapRequest(
      bearerToken: token,
      protocolMin: coreProtocolMin,
      protocolMax: coreProtocolMax,
      supervisorProcessId: pid,
      supervisionNonce: _newUuid(),
    );

    try {
      process.stdin.writeln(jsonEncode(request.toJson()));
      await process.stdin.flush();
      final hasLine = await stdoutLines.moveNext().timeout(startupTimeout);
      if (!hasLine) {
        throw const SidecarFailure(
          SidecarFailureKind.startupFailed,
          'CORE_BOOTSTRAP_CLOSED',
        );
      }
      final ready = BootstrapReady.fromJson(
        _decodeObject(stdoutLines.current, 'core bootstrap'),
      );
      if (ready.schemaVersion != coreSchemaVersion ||
          ready.port <= 0 ||
          ready.port > 65535) {
        throw const SidecarFailure(
          SidecarFailureKind.invalidResponse,
          'CORE_BOOTSTRAP_INVALID',
        );
      }

      return IoCoreSidecarSession(
        process,
        token,
        Uri(scheme: 'http', host: '127.0.0.1', port: ready.port),
        ready.instanceId,
        () => exited,
        requestTimeout: requestTimeout,
        runtimeEventInactivityTimeout: runtimeEventInactivityTimeout,
        shutdownTimeout: shutdownTimeout,
      );
    } on TimeoutException {
      await _terminateProcess(process);
      throw const SidecarFailure(
        SidecarFailureKind.startupTimedOut,
        'CORE_STARTUP_TIMEOUT',
      );
    } on SidecarFailure {
      await _terminateProcess(process);
      rethrow;
    } on Object {
      await _terminateProcess(process);
      throw const SidecarFailure(
        SidecarFailureKind.startupFailed,
        'CORE_STARTUP_FAILED',
      );
    } finally {
      unawaited(stdoutLines.cancel());
    }
  }
}

class IoCoreSidecarSession implements CoreSidecarSession {
  IoCoreSidecarSession(
    this._process,
    this._bearerToken,
    this._endpoint,
    this._instanceId,
    this._hasExited, {
    required this.requestTimeout,
    this.runtimeEventInactivityTimeout = _defaultRuntimeEventInactivityTimeout,
    required this.shutdownTimeout,
  }) : _client = HttpClient() {
    if (_endpoint.host != '127.0.0.1') {
      throw const SidecarFailure(
        SidecarFailureKind.invalidResponse,
        'CORE_ENDPOINT_NOT_LOOPBACK',
      );
    }
    _client.connectionTimeout = requestTimeout;
    _client.autoUncompress = false;
  }

  static const int _maxResponseBytes = 64 * 1024;
  static const int _maxEventStreamBytes = 512 * 1024;
  static const int _maxEvents = 64;

  final Process _process;
  final String _bearerToken;
  final Uri _endpoint;
  final InstanceId _instanceId;
  final bool Function() _hasExited;
  final HttpClient _client;
  final Duration requestTimeout;
  final Duration runtimeEventInactivityTimeout;
  final Duration shutdownTimeout;
  var _closed = false;

  @override
  bool get hasExited => _hasExited();

  @override
  Future<CoreHello> handshake(ClientHello hello) async {
    final response = await _post(
      '/internal/v1/handshake',
      hello.toJson(),
      hello.correlationId,
      hello.requestId,
    );
    final core = CoreHello.fromJson(response);
    if (core.instanceId != _instanceId ||
        core.correlationId != hello.correlationId ||
        core.requestId != hello.requestId) {
      throw const SidecarFailure(
        SidecarFailureKind.invalidResponse,
        'CORE_HANDSHAKE_ID_MISMATCH',
      );
    }
    return core;
  }

  @override
  Future<HealthResponse> health(HealthRequest request) async {
    final response = HealthResponse.fromJson(
      await _post(
        '/internal/v1/health',
        request.toJson(),
        request.correlationId,
        request.requestId,
      ),
    );
    _verifyIds(
      response.correlationId,
      response.requestId,
      request.correlationId,
      request.requestId,
    );
    return response;
  }

  @override
  Future<TestOperationStartResponse> startOperation(
    TestOperationStartRequest request,
  ) async {
    final response = TestOperationStartResponse.fromJson(
      await _post(
        '/internal/v1/test-operations',
        request.toJson(),
        request.correlationId,
        request.requestId,
      ),
    );
    _verifyIds(
      response.correlationId,
      response.requestId,
      request.correlationId,
      request.requestId,
    );
    return response;
  }

  @override
  Stream<TestOperationEvent> operationEvents(
    OperationId operationId,
    CorrelationId correlationId,
    RequestId requestId,
  ) async* {
    HttpClientResponse response;
    try {
      final request = await _client
          .getUrl(
            _endpoint.resolve(
              '/internal/v1/test-operations/$operationId/events',
            ),
          )
          .timeout(requestTimeout);
      _applyHeaders(request.headers, correlationId, requestId);
      response = await request.close().timeout(requestTimeout);
    } on TimeoutException {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'CORE_EVENT_CONNECTION_TIMEOUT',
      );
    } on SocketException {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'CORE_EVENT_CONNECTION_LOST',
      );
    } on HttpException {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'CORE_EVENT_CONNECTION_LOST',
      );
    }

    if (response.statusCode != HttpStatus.ok) {
      throw await _failureFromResponse(response, correlationId, requestId);
    }

    var expectedSequence = 1;
    var eventCount = 0;
    var terminal = false;
    try {
      await for (final line
          in response.transform(utf8.decoder).transform(const LineSplitter())) {
        if (!line.startsWith('data:')) {
          continue;
        }
        final data = line.substring(5).trimLeft();
        if (data.length > _maxResponseBytes || eventCount >= _maxEvents) {
          throw const SidecarFailure(
            SidecarFailureKind.invalidResponse,
            'CORE_EVENT_STREAM_LIMIT',
          );
        }
        final event = TestOperationEvent.fromJson(
          _decodeObject(data, 'core event'),
        );
        if (event.operationId != operationId ||
            event.correlationId != correlationId ||
            event.sequence != expectedSequence) {
          throw const SidecarFailure(
            SidecarFailureKind.invalidResponse,
            'CORE_EVENT_SEQUENCE_INVALID',
          );
        }
        expectedSequence += 1;
        eventCount += 1;
        terminal = event.terminalState != null;
        yield event;
        if (terminal) {
          return;
        }
      }
    } on FormatException {
      throw const SidecarFailure(
        SidecarFailureKind.invalidResponse,
        'CORE_EVENT_INVALID',
      );
    } on SocketException {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'CORE_EVENT_CONNECTION_LOST',
      );
    } on HttpException {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'CORE_EVENT_CONNECTION_LOST',
      );
    }
    if (!terminal) {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'CORE_EVENT_STREAM_ENDED',
      );
    }
  }

  @override
  Future<CancelOperationResponse> cancelOperation(
    OperationId operationId,
    CancelOperationRequest request,
  ) async {
    final response = CancelOperationResponse.fromJson(
      await _post(
        '/internal/v1/test-operations/$operationId/cancel',
        request.toJson(),
        request.correlationId,
        request.requestId,
      ),
    );
    _verifyIds(
      response.correlationId,
      response.requestId,
      request.correlationId,
      request.requestId,
    );
    return response;
  }

  @override
  Future<HardwareScanStartResponse> startHardwareScan(
    HardwareScanStartRequest request,
  ) async {
    final response = HardwareScanStartResponse.fromJson(
      await _post(
        '/internal/v1/hardware-scans',
        request.toJson(),
        request.correlationId,
        request.requestId,
      ),
    );
    _verifyIds(
      response.correlationId,
      response.requestId,
      request.correlationId,
      request.requestId,
    );
    return response;
  }

  @override
  Stream<HardwareScanEvent> hardwareScanEvents(
    OperationId operationId,
    CorrelationId correlationId,
    RequestId requestId,
  ) async* {
    HttpClientResponse response;
    try {
      final request = await _client
          .getUrl(
            _endpoint.resolve(
              '/internal/v1/hardware-scans/$operationId/events',
            ),
          )
          .timeout(requestTimeout);
      _applyHeaders(request.headers, correlationId, requestId);
      response = await request.close().timeout(requestTimeout);
    } on TimeoutException {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'HARDWARE_SCAN_CONNECTION_TIMEOUT',
      );
    } on SocketException {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'HARDWARE_SCAN_CONNECTION_LOST',
      );
    } on HttpException {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'HARDWARE_SCAN_CONNECTION_LOST',
      );
    }

    if (response.statusCode != HttpStatus.ok) {
      throw await _failureFromResponse(response, correlationId, requestId);
    }

    var expectedSequence = 1;
    var eventCount = 0;
    var terminal = false;
    try {
      await for (final line
          in response.transform(utf8.decoder).transform(const LineSplitter())) {
        if (!line.startsWith('data:')) {
          continue;
        }
        final data = line.substring(5).trimLeft();
        if (data.length > _maxResponseBytes || eventCount >= _maxEvents) {
          throw const SidecarFailure(
            SidecarFailureKind.invalidResponse,
            'HARDWARE_SCAN_STREAM_LIMIT',
          );
        }
        final event = HardwareScanEvent.fromJson(
          _decodeObject(data, 'hardware scan event'),
        );
        if (event.operationId != operationId ||
            event.correlationId != correlationId ||
            event.sequence != expectedSequence) {
          throw const SidecarFailure(
            SidecarFailureKind.invalidResponse,
            'HARDWARE_SCAN_SEQUENCE_INVALID',
          );
        }
        expectedSequence += 1;
        eventCount += 1;
        terminal = event.terminalState != null;
        yield event;
        if (terminal) {
          return;
        }
      }
    } on FormatException {
      throw const SidecarFailure(
        SidecarFailureKind.invalidResponse,
        'HARDWARE_SCAN_EVENT_INVALID',
      );
    } on SocketException {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'HARDWARE_SCAN_CONNECTION_LOST',
      );
    } on HttpException {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'HARDWARE_SCAN_CONNECTION_LOST',
      );
    }
    if (!terminal) {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'HARDWARE_SCAN_STREAM_ENDED',
      );
    }
  }

  @override
  Future<CancelOperationResponse> cancelHardwareScan(
    OperationId operationId,
    CancelOperationRequest request,
  ) async {
    final response = CancelOperationResponse.fromJson(
      await _post(
        '/internal/v1/hardware-scans/$operationId/cancel',
        request.toJson(),
        request.correlationId,
        request.requestId,
      ),
    );
    _verifyIds(
      response.correlationId,
      response.requestId,
      request.correlationId,
      request.requestId,
    );
    return response;
  }

  @override
  Future<RecommendationResponse> recommend(
    RecommendationRequest request,
  ) async {
    final response = RecommendationResponse.fromJson(
      await _post(
        '/internal/v1/recommendations',
        request.toJson(),
        request.correlationId,
        request.requestId,
      ),
    );
    _verifyIds(
      response.correlationId,
      response.requestId,
      request.correlationId,
      request.requestId,
    );
    return response;
  }

  @override
  Future<RuntimeStatusResponse> runtimeStatus(
    RuntimeStatusRequest request,
  ) async {
    final response = RuntimeStatusResponse.fromJson(
      await _post(
        '/internal/v1/runtime/status',
        request.toJson(),
        request.correlationId,
        request.requestId,
      ),
    );
    _verifyIds(
      response.correlationId,
      response.requestId,
      request.correlationId,
      request.requestId,
    );
    return response;
  }

  @override
  Future<RuntimeConsentResponse> runtimeConsent(
    RuntimeConsentRequest request,
  ) async {
    final response = RuntimeConsentResponse.fromJson(
      await _post(
        '/internal/v1/runtime/consent',
        request.toJson(),
        request.correlationId,
        request.requestId,
      ),
    );
    _verifyIds(
      response.correlationId,
      response.requestId,
      request.correlationId,
      request.requestId,
    );
    return response;
  }

  @override
  Future<RuntimeModelInventoryResponse> runtimeModels(
    RuntimeModelInventoryRequest request,
  ) async {
    final response = RuntimeModelInventoryResponse.fromJson(
      await _post(
        '/internal/v1/runtime/models',
        request.toJson(),
        request.correlationId,
        request.requestId,
      ),
    );
    _verifyIds(
      response.correlationId,
      response.requestId,
      request.correlationId,
      request.requestId,
    );
    return response;
  }

  @override
  Future<RuntimeOperationStartResponse> startRuntimeOperation(
    RuntimeOperationStartRequest request,
  ) async {
    final response = RuntimeOperationStartResponse.fromJson(
      await _post(
        '/internal/v1/runtime/operations',
        request.toJson(),
        request.correlationId,
        request.requestId,
      ),
    );
    _verifyIds(
      response.correlationId,
      response.requestId,
      request.correlationId,
      request.requestId,
    );
    return response;
  }

  @override
  Stream<RuntimeOperationEvent> runtimeOperationEvents(
    OperationId operationId,
    CorrelationId correlationId,
    RequestId requestId,
  ) async* {
    HttpClientResponse response;
    try {
      final request = await _client
          .getUrl(
            _endpoint.resolve(
              '/internal/v1/runtime/operations/$operationId/events',
            ),
          )
          .timeout(requestTimeout);
      _applyHeaders(request.headers, correlationId, requestId);
      response = await request.close().timeout(requestTimeout);
    } on TimeoutException {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'RUNTIME_OPERATION_CONNECTION_TIMEOUT',
      );
    } on SocketException {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'RUNTIME_OPERATION_CONNECTION_LOST',
      );
    } on HttpException {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'RUNTIME_OPERATION_CONNECTION_LOST',
      );
    }

    if (response.statusCode != HttpStatus.ok) {
      throw await _failureFromResponse(response, correlationId, requestId);
    }

    var expectedSequence = 1;
    var eventCount = 0;
    var terminal = false;
    try {
      var streamedBytes = 0;
      final bounded = response
          .timeout(runtimeEventInactivityTimeout)
          .transform(
            StreamTransformer<List<int>, List<int>>.fromHandlers(
              handleData: (chunk, sink) {
                streamedBytes += chunk.length;
                if (streamedBytes > _maxEventStreamBytes) {
                  sink.addError(
                    const SidecarFailure(
                      SidecarFailureKind.invalidResponse,
                      'RUNTIME_OPERATION_STREAM_LIMIT',
                    ),
                  );
                  return;
                }
                sink.add(chunk);
              },
            ),
          );
      await for (final line
          in bounded.transform(utf8.decoder).transform(const LineSplitter())) {
        if (!line.startsWith('data:')) {
          continue;
        }
        final data = line.substring(5).trimLeft();
        if (data.length > _maxResponseBytes || eventCount >= _maxEvents) {
          throw const SidecarFailure(
            SidecarFailureKind.invalidResponse,
            'RUNTIME_OPERATION_STREAM_LIMIT',
          );
        }
        final event = RuntimeOperationEvent.fromJson(
          _decodeObject(data, 'runtime operation event'),
        );
        if (event.operationId != operationId ||
            event.correlationId != correlationId ||
            event.sequence != expectedSequence) {
          throw const SidecarFailure(
            SidecarFailureKind.invalidResponse,
            'RUNTIME_OPERATION_SEQUENCE_INVALID',
          );
        }
        expectedSequence += 1;
        eventCount += 1;
        terminal = event.terminalState != null;
        yield event;
        if (terminal) {
          return;
        }
      }
    } on TimeoutException {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'RUNTIME_OPERATION_STREAM_TIMEOUT',
      );
    } on FormatException {
      throw const SidecarFailure(
        SidecarFailureKind.invalidResponse,
        'RUNTIME_OPERATION_EVENT_INVALID',
      );
    } on SocketException {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'RUNTIME_OPERATION_CONNECTION_LOST',
      );
    } on HttpException {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'RUNTIME_OPERATION_CONNECTION_LOST',
      );
    }
    if (!terminal) {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'RUNTIME_OPERATION_STREAM_ENDED',
      );
    }
  }

  @override
  Future<CancelOperationResponse> cancelRuntimeOperation(
    OperationId operationId,
    CancelOperationRequest request,
  ) async {
    final response = CancelOperationResponse.fromJson(
      await _post(
        '/internal/v1/runtime/operations/$operationId/cancel',
        request.toJson(),
        request.correlationId,
        request.requestId,
      ),
    );
    _verifyIds(
      response.correlationId,
      response.requestId,
      request.correlationId,
      request.requestId,
    );
    return response;
  }

  @override
  Future<void> shutdown(ShutdownRequest request) async {
    if (_closed) {
      return;
    }
    _closed = true;
    try {
      final response = ShutdownResponse.fromJson(
        await _post(
          '/internal/v1/shutdown',
          request.toJson(),
          request.correlationId,
          request.requestId,
          allowClosed: true,
        ).timeout(shutdownTimeout),
      );
      _verifyIds(
        response.correlationId,
        response.requestId,
        request.correlationId,
        request.requestId,
      );
    } on Object {
      // The bounded process wait below remains the authority for cleanup.
    } finally {
      _client.close(force: true);
      try {
        await _process.stdin.close();
      } on Object {
        // The process may have already closed its inherited pipe.
      }
    }

    try {
      await _process.exitCode.timeout(shutdownTimeout);
    } on TimeoutException {
      _process.kill();
      try {
        await _process.exitCode.timeout(const Duration(seconds: 1));
      } on TimeoutException {
        // Windows can delay process reaping; no unbounded await is retained.
      }
    }
  }

  Future<Map<String, dynamic>> _post(
    String path,
    Map<String, dynamic> body,
    CorrelationId correlationId,
    RequestId requestId, {
    bool allowClosed = false,
  }) async {
    if (_closed && !allowClosed) {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'CORE_SESSION_CLOSED',
      );
    }
    try {
      final request = await _client
          .postUrl(_endpoint.resolve(path))
          .timeout(requestTimeout);
      _applyHeaders(request.headers, correlationId, requestId);
      request.headers.contentType = ContentType.json;
      request.add(utf8.encode(jsonEncode(body)));
      final response = await request.close().timeout(requestTimeout);
      if (response.statusCode < HttpStatus.ok ||
          response.statusCode >= HttpStatus.multipleChoices) {
        throw await _failureFromResponse(response, correlationId, requestId);
      }
      return _decodeObject(
        utf8.decode(await _readBounded(response)),
        'core response',
      );
    } on SidecarFailure {
      rethrow;
    } on TimeoutException {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'CORE_REQUEST_TIMEOUT',
      );
    } on SocketException {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'CORE_CONNECTION_LOST',
      );
    } on HttpException {
      throw const SidecarFailure(
        SidecarFailureKind.connectionLost,
        'CORE_CONNECTION_LOST',
      );
    } on FormatException {
      throw const SidecarFailure(
        SidecarFailureKind.invalidResponse,
        'CORE_RESPONSE_INVALID',
      );
    }
  }

  void _applyHeaders(
    HttpHeaders headers,
    CorrelationId correlationId,
    RequestId requestId,
  ) {
    headers.set(HttpHeaders.authorizationHeader, 'Bearer $_bearerToken');
    headers.set('x-gixgiz-correlation-id', correlationId);
    headers.set('x-gixgiz-request-id', requestId);
  }

  Future<SidecarFailure> _failureFromResponse(
    HttpClientResponse response,
    CorrelationId expectedCorrelationId,
    RequestId expectedRequestId,
  ) async {
    late final SafeErrorPayload error;
    try {
      error = SafeErrorPayload.fromJson(
        _decodeObject(utf8.decode(await _readBounded(response)), 'core error'),
      );
    } on TimeoutException {
      rethrow;
    } on Object {
      return const SidecarFailure(
        SidecarFailureKind.invalidResponse,
        'CORE_ERROR_RESPONSE_INVALID',
      );
    }
    _verifyIds(
      error.correlationId,
      error.requestId,
      expectedCorrelationId,
      expectedRequestId,
    );
    final kind = switch (error.code) {
      'transport.protocol_incompatible' => SidecarFailureKind.protocolMismatch,
      'transport.authentication_required' =>
        SidecarFailureKind.authenticationFailed,
      'core.operation_cancelled' => SidecarFailureKind.cancelled,
      _ when error.category == ErrorCategory.cancelled =>
        SidecarFailureKind.cancelled,
      _ => SidecarFailureKind.coreFailure,
    };
    return SidecarFailure(kind, error.code, safeError: error);
  }

  Future<List<int>> _readBounded(HttpClientResponse response) async {
    final bytes = <int>[];
    final chunks = StreamIterator<List<int>>(response);
    final elapsed = Stopwatch()..start();
    try {
      while (true) {
        final remaining = requestTimeout - elapsed.elapsed;
        if (remaining <= Duration.zero ||
            !await chunks.moveNext().timeout(remaining)) {
          if (remaining <= Duration.zero) {
            throw TimeoutException('core response body deadline elapsed');
          }
          break;
        }
        final chunk = chunks.current;
        if (bytes.length + chunk.length > _maxResponseBytes) {
          throw const SidecarFailure(
            SidecarFailureKind.invalidResponse,
            'CORE_RESPONSE_TOO_LARGE',
          );
        }
        bytes.addAll(chunk);
      }
    } finally {
      elapsed.stop();
      await chunks.cancel();
    }
    return bytes;
  }

  void _verifyIds(
    CorrelationId actualCorrelation,
    RequestId actualRequest,
    CorrelationId expectedCorrelation,
    RequestId expectedRequest,
  ) {
    if (actualCorrelation != expectedCorrelation ||
        actualRequest != expectedRequest) {
      throw const SidecarFailure(
        SidecarFailureKind.invalidResponse,
        'CORE_RESPONSE_ID_MISMATCH',
      );
    }
  }
}

String bundledCorePath() {
  final directory = File(Platform.resolvedExecutable).parent.path;
  return '$directory${Platform.pathSeparator}gixgiz-core.exe';
}

Map<String, dynamic> _decodeObject(String value, String label) {
  final decoded = jsonDecode(value);
  if (decoded is! Map<String, dynamic>) {
    throw FormatException('$label must be a JSON object.');
  }
  return decoded;
}

String _secureHex(int byteCount) {
  final random = Random.secure();
  final buffer = StringBuffer();
  for (var index = 0; index < byteCount; index += 1) {
    buffer.write(random.nextInt(256).toRadixString(16).padLeft(2, '0'));
  }
  return buffer.toString();
}

String _newUuid() {
  final random = Random.secure();
  final bytes = List<int>.generate(16, (_) => random.nextInt(256));
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  final hex = bytes
      .map((byte) => byte.toRadixString(16).padLeft(2, '0'))
      .join();
  return '${hex.substring(0, 8)}-${hex.substring(8, 12)}-'
      '${hex.substring(12, 16)}-${hex.substring(16, 20)}-'
      '${hex.substring(20)}';
}

String newCorrelationId() => _newUuid();

String newRequestId() => _newUuid();

Future<void> _terminateProcess(Process process) async {
  process.kill();
  try {
    await process.stdin.close();
  } on Object {
    // A failed process can close its pipe before the desktop does.
  }
  try {
    await process.exitCode.timeout(const Duration(seconds: 1));
  } on TimeoutException {
    process.kill(ProcessSignal.sigkill);
  }
}
