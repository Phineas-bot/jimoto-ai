import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';
import 'package:gixgiz_desktop/core/sidecar_connection.dart';

import 'support/setup_fixture.dart';

void main() {
  test('runtime status uses the authenticated loopback route', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    final handledStatus = Completer<void>();
    final subscription = server.listen((request) async {
      if (request.uri.path == '/internal/v1/shutdown') {
        await _respondToShutdown(request);
        return;
      }

      expect(request.method, 'POST');
      expect(request.uri.path, '/internal/v1/runtime/status');
      expect(
        request.headers.value(HttpHeaders.authorizationHeader),
        'Bearer test-token',
      );
      final body = jsonDecode(await utf8.decoder.bind(request).join());
      final runtimeRequest = RuntimeStatusRequest.fromJson(
        (body as Map).cast<String, dynamic>(),
      );
      expect(runtimeRequest.providerId, 'gixgiz.runtime.ollama.v1');
      expect(
        request.headers.value('x-gixgiz-correlation-id'),
        runtimeRequest.correlationId,
      );
      expect(
        request.headers.value('x-gixgiz-request-id'),
        runtimeRequest.requestId,
      );

      request.response.headers.contentType = ContentType.json;
      request.response.write(
        jsonEncode(
          RuntimeStatusResponse(
            report: _runtimeReport,
            correlationId: runtimeRequest.correlationId,
            requestId: runtimeRequest.requestId,
          ).toJson(),
        ),
      );
      await request.response.close();
      handledStatus.complete();
    });
    addTearDown(() async {
      await subscription.cancel();
      await server.close(force: true);
    });
    final process = _FakeProcess();
    final session = _session(server, process);

    final response = await session.runtimeStatus(
      const RuntimeStatusRequest(
        providerId: 'gixgiz.runtime.ollama.v1',
        correlationId: '00000000-0000-4000-8000-000000000001',
        requestId: '00000000-0000-4000-8000-000000000002',
      ),
    );

    expect(response.report.state, RuntimeState.installedStopped);
    await handledStatus.future;
    await _shutdown(session);
  });

  test('runtime safe error accepts matching correlation identifiers', () async {
    const correlationId = '00000000-0000-4000-8000-000000000020';
    const requestId = '00000000-0000-4000-8000-000000000021';
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    final subscription = server.listen((request) async {
      if (request.uri.path == '/internal/v1/shutdown') {
        await _respondToShutdown(request);
        return;
      }
      await utf8.decoder.bind(request).join();
      request.response.statusCode = HttpStatus.serviceUnavailable;
      request.response.headers.contentType = ContentType.json;
      request.response.write(
        jsonEncode(
          _safeError(
            correlationId: correlationId,
            requestId: requestId,
          ).toJson(),
        ),
      );
      await request.response.close();
    });
    addTearDown(() async {
      await subscription.cancel();
      await server.close(force: true);
    });
    final session = _session(server, _FakeProcess());

    await expectLater(
      session.runtimeStatus(
        const RuntimeStatusRequest(
          providerId: 'gixgiz.runtime.ollama.v1',
          correlationId: correlationId,
          requestId: requestId,
        ),
      ),
      throwsA(
        isA<SidecarFailure>()
            .having(
              (failure) => failure.diagnosticCode,
              'diagnosticCode',
              'runtime.endpoint_unavailable',
            )
            .having(
              (failure) => failure.safeError?.category,
              'category',
              ErrorCategory.unavailable,
            ),
      ),
    );
    await _shutdown(session);
  });

  test(
    'runtime SSE error rejects mismatched correlation identifiers',
    () async {
      const correlationId = '00000000-0000-4000-8000-000000000022';
      const requestId = '00000000-0000-4000-8000-000000000023';
      final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
      final subscription = server.listen((request) async {
        if (request.uri.path == '/internal/v1/shutdown') {
          await _respondToShutdown(request);
          return;
        }
        request.response.statusCode = HttpStatus.serviceUnavailable;
        request.response.headers.contentType = ContentType.json;
        request.response.write(
          jsonEncode(
            _safeError(
              correlationId: '00000000-0000-4000-8000-000000000024',
              requestId: requestId,
            ).toJson(),
          ),
        );
        await request.response.close();
      });
      addTearDown(() async {
        await subscription.cancel();
        await server.close(force: true);
      });
      final session = _session(server, _FakeProcess());

      await expectLater(
        session.runtimeOperationEvents(
          'operation-error',
          correlationId,
          requestId,
        ),
        emitsError(
          isA<SidecarFailure>()
              .having(
                (failure) => failure.kind,
                'kind',
                SidecarFailureKind.invalidResponse,
              )
              .having(
                (failure) => failure.diagnosticCode,
                'diagnosticCode',
                'CORE_RESPONSE_ID_MISMATCH',
              ),
        ),
      );
      await _shutdown(session);
    },
  );

  test('malformed runtime SSE event becomes a stable failure', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    final subscription = server.listen((request) async {
      if (request.uri.path == '/internal/v1/shutdown') {
        await _respondToShutdown(request);
        return;
      }
      request.response.statusCode = HttpStatus.ok;
      request.response.headers.contentType = ContentType(
        'text',
        'event-stream',
      );
      request.response.write('data: {not-json}\n\n');
      await request.response.close();
    });
    addTearDown(() async {
      await subscription.cancel();
      await server.close(force: true);
    });
    final session = _session(server, _FakeProcess());

    await expectLater(
      session.runtimeOperationEvents(
        'operation-malformed',
        '00000000-0000-4000-8000-000000000025',
        '00000000-0000-4000-8000-000000000026',
      ),
      emitsError(
        isA<SidecarFailure>().having(
          (failure) => failure.diagnosticCode,
          'diagnosticCode',
          'RUNTIME_OPERATION_EVENT_INVALID',
        ),
      ),
    );
    await _shutdown(session);
  });

  test('runtime event stream rejects a cumulative oversized body', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    final subscription = server.listen((request) async {
      if (request.uri.path == '/internal/v1/shutdown') {
        await _respondToShutdown(request);
        return;
      }
      expect(
        request.uri.path,
        '/internal/v1/runtime/operations/operation-1/events',
      );
      request.response.statusCode = HttpStatus.ok;
      request.response.headers.contentType = ContentType(
        'text',
        'event-stream',
      );
      request.response.write('x' * (512 * 1024 + 1));
      try {
        await request.response.close();
      } on Object {
        // The client deliberately closes as soon as the bound is exceeded.
      }
    });
    addTearDown(() async {
      await subscription.cancel();
      await server.close(force: true);
    });
    final process = _FakeProcess();
    final session = _session(server, process);

    await expectLater(
      session.runtimeOperationEvents(
        'operation-1',
        '00000000-0000-4000-8000-000000000003',
        '00000000-0000-4000-8000-000000000004',
      ),
      emitsError(
        isA<SidecarFailure>().having(
          (failure) => failure.diagnosticCode,
          'diagnosticCode',
          'RUNTIME_OPERATION_STREAM_LIMIT',
        ),
      ),
    );
    await _shutdown(session);
  });

  test('runtime event stream applies an inactivity timeout', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    final subscription = server.listen((request) async {
      if (request.uri.path == '/internal/v1/shutdown') {
        await _respondToShutdown(request);
        return;
      }
      request.response.statusCode = HttpStatus.ok;
      request.response.headers.contentType = ContentType(
        'text',
        'event-stream',
      );
      request.response.write(': connected\n\n');
      await request.response.flush();
      await Future<void>.delayed(const Duration(milliseconds: 200));
      try {
        await request.response.close();
      } on Object {
        // The inactivity timeout closes the client connection first.
      }
    });
    addTearDown(() async {
      await subscription.cancel();
      await server.close(force: true);
    });
    final process = _FakeProcess();
    final session = _session(
      server,
      process,
      requestTimeout: const Duration(milliseconds: 50),
      runtimeEventInactivityTimeout: const Duration(milliseconds: 50),
    );

    await expectLater(
      session.runtimeOperationEvents(
        'operation-2',
        '00000000-0000-4000-8000-000000000005',
        '00000000-0000-4000-8000-000000000006',
      ),
      emitsError(
        isA<SidecarFailure>().having(
          (failure) => failure.diagnosticCode,
          'diagnosticCode',
          'RUNTIME_OPERATION_STREAM_TIMEOUT',
        ),
      ),
    );
    await _shutdown(session);
  });

  test(
    'setup SSE sends a bounded exclusive cursor and accepts causal IDs',
    () async {
      const requestCorrelationId = '00000000-0000-4000-8000-000000000030';
      const requestId = '00000000-0000-4000-8000-000000000031';
      final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
      final subscription = server.listen((request) async {
        if (request.uri.path == '/internal/v1/shutdown') {
          await _respondToShutdown(request);
          return;
        }
        expect(request.method, 'GET');
        expect(
          request.uri.path,
          '/internal/v1/setup/jobs/$setupJobIdFixture/events',
        );
        expect(request.uri.queryParameters['after_sequence'], '7');
        expect(request.uri.queryParameters['limit'], '64');
        expect(
          request.headers.value('x-gixgiz-correlation-id'),
          requestCorrelationId,
        );
        expect(request.headers.value('x-gixgiz-request-id'), requestId);
        final event = setupEventFixture(
          sequence: 8,
          job: setupJobFixture(state: 'active', stage: 'acquiring'),
        );
        request.response.statusCode = HttpStatus.ok;
        request.response.headers.contentType = ContentType(
          'text',
          'event-stream',
        );
        request.response.write('data: ${jsonEncode(event.toJson())}\n\n');
        await request.response.close();
      });
      addTearDown(() async {
        await subscription.cancel();
        await server.close(force: true);
      });
      final session = _session(server, _FakeProcess());

      final events = await session
          .setupJobEvents(
            const SetupJobEventsRequest(
              jobId: setupJobIdFixture,
              afterSequence: 7,
              limit: 64,
              correlationId: requestCorrelationId,
              requestId: requestId,
            ),
          )
          .toList();

      expect(events, hasLength(1));
      expect(events.single.sequence, 8);
      expect(events.single.correlationId, setupCorrelationIdFixture);
      await _shutdown(session);
    },
  );

  test(
    'runtime event stream tolerates more than five seconds of silence',
    () async {
      const operationId = 'operation-slow';
      const correlationId = '00000000-0000-4000-8000-000000000027';
      const requestId = '00000000-0000-4000-8000-000000000028';
      final silentFor = Completer<Duration>();
      final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
      final subscription = server.listen((request) async {
        if (request.uri.path == '/internal/v1/shutdown') {
          await _respondToShutdown(request);
          return;
        }
        request.response.statusCode = HttpStatus.ok;
        request.response.headers.contentType = ContentType(
          'text',
          'event-stream',
        );
        request.response.write(': connected\n\n');
        await request.response.flush();
        final elapsed = Stopwatch()..start();
        await Future<void>.delayed(const Duration(milliseconds: 5200));
        elapsed.stop();
        silentFor.complete(elapsed.elapsed);
        final terminalEvent = RuntimeOperationEvent(
          schemaVersion: 1,
          operationId: operationId,
          correlationId: correlationId,
          sequence: 1,
          operationKind: RuntimeOperationKind.start,
          kind: RuntimeOperationEventKind.completed,
          timestampUnixMs: 1,
          message: 'completed',
          report: _runtimeReport,
          error: null,
          terminalState: RuntimeOperationTerminalState.completed,
        );
        request.response.write(
          'data: ${jsonEncode(terminalEvent.toJson())}\n\n',
        );
        await request.response.close();
      });
      addTearDown(() async {
        await subscription.cancel();
        await server.close(force: true);
      });
      final session = _session(
        server,
        _FakeProcess(),
        requestTimeout: const Duration(seconds: 5),
      );

      final events = await session
          .runtimeOperationEvents(operationId, correlationId, requestId)
          .toList();

      expect(await silentFor.future, greaterThan(const Duration(seconds: 5)));
      expect(events, hasLength(1));
      expect(
        events.single.terminalState,
        RuntimeOperationTerminalState.completed,
      );
      await _shutdown(session);
    },
  );

  test('runtime JSON success body has an absolute response deadline', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    final subscription = server.listen((request) async {
      if (request.uri.path == '/internal/v1/shutdown') {
        await _respondToShutdown(request);
        return;
      }
      request.response.statusCode = HttpStatus.ok;
      request.response.headers.contentType = ContentType.json;
      request.response.write('{');
      await request.response.flush();
      await Future<void>.delayed(const Duration(milliseconds: 200));
      try {
        await request.response.close();
      } on Object {
        // The absolute body deadline closes the client connection first.
      }
    });
    addTearDown(() async {
      await subscription.cancel();
      await server.close(force: true);
    });
    final session = _session(
      server,
      _FakeProcess(),
      requestTimeout: const Duration(milliseconds: 50),
    );

    await expectLater(
      session.runtimeStatus(
        const RuntimeStatusRequest(
          providerId: 'gixgiz.runtime.ollama.v1',
          correlationId: '00000000-0000-4000-8000-000000000012',
          requestId: '00000000-0000-4000-8000-000000000013',
        ),
      ),
      throwsA(
        isA<SidecarFailure>().having(
          (failure) => failure.diagnosticCode,
          'diagnosticCode',
          'CORE_REQUEST_TIMEOUT',
        ),
      ),
    );
    await _shutdown(session);
  });

  test('runtime JSON error body has an absolute response deadline', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    final subscription = server.listen((request) async {
      if (request.uri.path == '/internal/v1/shutdown') {
        await _respondToShutdown(request);
        return;
      }
      request.response.statusCode = HttpStatus.serviceUnavailable;
      request.response.headers.contentType = ContentType.json;
      request.response.write('{');
      await request.response.flush();
      await Future<void>.delayed(const Duration(milliseconds: 200));
      try {
        await request.response.close();
      } on Object {
        // The absolute body deadline closes the client connection first.
      }
    });
    addTearDown(() async {
      await subscription.cancel();
      await server.close(force: true);
    });
    final session = _session(
      server,
      _FakeProcess(),
      requestTimeout: const Duration(milliseconds: 50),
    );

    await expectLater(
      session.runtimeStatus(
        const RuntimeStatusRequest(
          providerId: 'gixgiz.runtime.ollama.v1',
          correlationId: '00000000-0000-4000-8000-000000000014',
          requestId: '00000000-0000-4000-8000-000000000015',
        ),
      ),
      throwsA(
        isA<SidecarFailure>().having(
          (failure) => failure.diagnosticCode,
          'diagnosticCode',
          'CORE_REQUEST_TIMEOUT',
        ),
      ),
    );
    await _shutdown(session);
  });
}

IoCoreSidecarSession _session(
  HttpServer server,
  Process process, {
  Duration requestTimeout = const Duration(seconds: 1),
  Duration runtimeEventInactivityTimeout = const Duration(seconds: 40),
}) {
  return IoCoreSidecarSession(
    process,
    'test-token',
    Uri.parse('http://127.0.0.1:${server.port}'),
    '00000000-0000-4000-8000-000000000000',
    () => false,
    requestTimeout: requestTimeout,
    runtimeEventInactivityTimeout: runtimeEventInactivityTimeout,
    shutdownTimeout: const Duration(seconds: 1),
  );
}

Future<void> _shutdown(IoCoreSidecarSession session) {
  return session.shutdown(
    const ShutdownRequest(
      correlationId: '00000000-0000-4000-8000-000000000007',
      requestId: '00000000-0000-4000-8000-000000000008',
    ),
  );
}

Future<void> _respondToShutdown(HttpRequest request) async {
  final body = jsonDecode(await utf8.decoder.bind(request).join());
  final shutdown = ShutdownRequest.fromJson(
    (body as Map).cast<String, dynamic>(),
  );
  request.response.headers.contentType = ContentType.json;
  request.response.write(
    jsonEncode(
      ShutdownResponse(
        accepted: true,
        correlationId: shutdown.correlationId,
        requestId: shutdown.requestId,
      ).toJson(),
    ),
  );
  await request.response.close();
}

SafeErrorPayload _safeError({
  required CorrelationId correlationId,
  required RequestId requestId,
}) {
  return SafeErrorPayload(
    category: ErrorCategory.unavailable,
    code: 'runtime.endpoint_unavailable',
    correlationId: correlationId,
    message: 'The runtime endpoint is unavailable.',
    recovery: const RecoveryGuidance(
      action: RecoveryAction.retry,
      message: 'Try the runtime check again.',
    ),
    requestId: requestId,
  );
}

final RuntimeHealthReport _runtimeReport = RuntimeHealthReport.fromJson({
  'schema_version': 1,
  'provider_id': 'gixgiz.runtime.ollama.v1',
  'display_name': 'Fixture runtime',
  'state': 'installed_stopped',
  'ownership': 'external',
  'reuse_consent': 'not_requested',
  'management_consent': 'not_requested',
  'endpoint_safety': 'loopback_verified',
  'version': null,
  'capabilities': <Object?>[],
  'reasons': <Object?>[],
  'warnings': <Object?>[],
});

class _FakeProcess implements Process {
  _FakeProcess()
    : _inputController = StreamController<List<int>>.broadcast(),
      _exitCode = Future<int>.value(0) {
    _stdin = IOSink(_inputController.sink);
  }

  final StreamController<List<int>> _inputController;
  final Future<int> _exitCode;
  late final IOSink _stdin;

  @override
  Future<int> get exitCode => _exitCode;

  @override
  int get pid => 1;

  @override
  Stream<List<int>> get stderr => const Stream<List<int>>.empty();

  @override
  IOSink get stdin => _stdin;

  @override
  Stream<List<int>> get stdout => const Stream<List<int>>.empty();

  @override
  bool kill([ProcessSignal signal = ProcessSignal.sigterm]) => true;
}
