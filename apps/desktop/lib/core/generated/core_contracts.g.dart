// GENERATED CODE - DO NOT MODIFY BY HAND.
// Source: crates/gixgiz-contracts and schemas/gixgiz-transport.schema.json
// Regenerate: cargo run -p gixgiz-contracts --example generate_bindings -- --write

// ignore_for_file: unnecessary_lambdas

class AccelerationEvidence {
  const AccelerationEvidence({
    required this.kind,
    required this.metadata,
    required this.supported,
  });

  final AccelerationKind kind;
  final EvidenceMetadata metadata;
  final bool? supported;

  factory AccelerationEvidence.fromJson(Map<String, dynamic> json) {
    return AccelerationEvidence(
      kind: AccelerationKind.fromJson(json['kind']),
      metadata: EvidenceMetadata.fromJson(_contractMap(json['metadata'], 'AccelerationEvidence.metadata')),
      supported: json['supported'] == null ? null : _contractBool(json['supported'], 'AccelerationEvidence.supported'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'kind': kind.toJson(),
      'metadata': metadata.toJson(),
      'supported': supported,
    };
  }
}

enum AccelerationKind {
  directMl('direct_ml'),
  cuda('cuda'),
  rocm('rocm'),
  unknown('unknown'),
  ;

  const AccelerationKind(this.wireValue);

  final String wireValue;

  static AccelerationKind fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class ApplicationInfo {
  const ApplicationInfo({
    required this.applicationId,
    required this.name,
    required this.protocolVersion,
    required this.schemaVersion,
    required this.version,
  });

  final String applicationId;
  final String name;
  final int protocolVersion;
  final int schemaVersion;
  final String version;

  factory ApplicationInfo.fromJson(Map<String, dynamic> json) {
    return ApplicationInfo(
      applicationId: _contractString(json['application_id'], 'ApplicationInfo.application_id'),
      name: _contractString(json['name'], 'ApplicationInfo.name'),
      protocolVersion: _contractInt(json['protocol_version'], 'ApplicationInfo.protocol_version'),
      schemaVersion: _contractInt(json['schema_version'], 'ApplicationInfo.schema_version'),
      version: _contractString(json['version'], 'ApplicationInfo.version'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'application_id': applicationId,
      'name': name,
      'protocol_version': protocolVersion,
      'schema_version': schemaVersion,
      'version': version,
    };
  }
}

class ArchitectureEvidence {
  const ArchitectureEvidence({
    required this.metadata,
    required this.value,
  });

  final EvidenceMetadata metadata;
  final MachineArchitecture? value;

  factory ArchitectureEvidence.fromJson(Map<String, dynamic> json) {
    return ArchitectureEvidence(
      metadata: EvidenceMetadata.fromJson(_contractMap(json['metadata'], 'ArchitectureEvidence.metadata')),
      value: json['value'] == null ? null : MachineArchitecture.fromJson(json['value']),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'metadata': metadata.toJson(),
      'value': value?.toJson(),
    };
  }
}

class BootstrapReady {
  const BootstrapReady({
    required this.instanceId,
    required this.port,
    required this.protocolMax,
    required this.protocolMin,
    required this.schemaVersion,
  });

  final InstanceId instanceId;
  final int port;
  final int protocolMax;
  final int protocolMin;
  final int schemaVersion;

  factory BootstrapReady.fromJson(Map<String, dynamic> json) {
    return BootstrapReady(
      instanceId: _contractString(json['instance_id'], 'BootstrapReady.instance_id'),
      port: _contractInt(json['port'], 'BootstrapReady.port'),
      protocolMax: _contractInt(json['protocol_max'], 'BootstrapReady.protocol_max'),
      protocolMin: _contractInt(json['protocol_min'], 'BootstrapReady.protocol_min'),
      schemaVersion: _contractInt(json['schema_version'], 'BootstrapReady.schema_version'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'instance_id': instanceId,
      'port': port,
      'protocol_max': protocolMax,
      'protocol_min': protocolMin,
      'schema_version': schemaVersion,
    };
  }
}

class BootstrapRequest {
  const BootstrapRequest({
    required this.bearerToken,
    required this.protocolMax,
    required this.protocolMin,
    required this.supervisionNonce,
    required this.supervisorProcessId,
  });

  final String bearerToken;
  final int protocolMax;
  final int protocolMin;
  final SupervisionNonce supervisionNonce;
  final int supervisorProcessId;

  factory BootstrapRequest.fromJson(Map<String, dynamic> json) {
    return BootstrapRequest(
      bearerToken: _contractString(json['bearer_token'], 'BootstrapRequest.bearer_token'),
      protocolMax: _contractInt(json['protocol_max'], 'BootstrapRequest.protocol_max'),
      protocolMin: _contractInt(json['protocol_min'], 'BootstrapRequest.protocol_min'),
      supervisionNonce: _contractString(json['supervision_nonce'], 'BootstrapRequest.supervision_nonce'),
      supervisorProcessId: _contractInt(json['supervisor_process_id'], 'BootstrapRequest.supervisor_process_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'bearer_token': bearerToken,
      'protocol_max': protocolMax,
      'protocol_min': protocolMin,
      'supervision_nonce': supervisionNonce,
      'supervisor_process_id': supervisorProcessId,
    };
  }
}

class CancelOperationRequest {
  const CancelOperationRequest({
    required this.correlationId,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final RequestId requestId;

  factory CancelOperationRequest.fromJson(Map<String, dynamic> json) {
    return CancelOperationRequest(
      correlationId: _contractString(json['correlation_id'], 'CancelOperationRequest.correlation_id'),
      requestId: _contractString(json['request_id'], 'CancelOperationRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'request_id': requestId,
    };
  }
}

class CancelOperationResponse {
  const CancelOperationResponse({
    required this.accepted,
    required this.correlationId,
    required this.operationId,
    required this.requestId,
  });

  final bool accepted;
  final CorrelationId correlationId;
  final OperationId operationId;
  final RequestId requestId;

  factory CancelOperationResponse.fromJson(Map<String, dynamic> json) {
    return CancelOperationResponse(
      accepted: _contractBool(json['accepted'], 'CancelOperationResponse.accepted'),
      correlationId: _contractString(json['correlation_id'], 'CancelOperationResponse.correlation_id'),
      operationId: _contractString(json['operation_id'], 'CancelOperationResponse.operation_id'),
      requestId: _contractString(json['request_id'], 'CancelOperationResponse.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'accepted': accepted,
      'correlation_id': correlationId,
      'operation_id': operationId,
      'request_id': requestId,
    };
  }
}

class ClientHello {
  const ClientHello({
    required this.clientName,
    required this.clientVersion,
    required this.correlationId,
    required this.protocolMax,
    required this.protocolMin,
    required this.requestId,
    required this.requestedCapabilities,
  });

  final String clientName;
  final String clientVersion;
  final CorrelationId correlationId;
  final int protocolMax;
  final int protocolMin;
  final RequestId requestId;
  final List<TransportCapability> requestedCapabilities;

  factory ClientHello.fromJson(Map<String, dynamic> json) {
    return ClientHello(
      clientName: _contractString(json['client_name'], 'ClientHello.client_name'),
      clientVersion: _contractString(json['client_version'], 'ClientHello.client_version'),
      correlationId: _contractString(json['correlation_id'], 'ClientHello.correlation_id'),
      protocolMax: _contractInt(json['protocol_max'], 'ClientHello.protocol_max'),
      protocolMin: _contractInt(json['protocol_min'], 'ClientHello.protocol_min'),
      requestId: _contractString(json['request_id'], 'ClientHello.request_id'),
      requestedCapabilities: _contractList(json['requested_capabilities'], 'ClientHello.requested_capabilities').map((item) => TransportCapability.fromJson(item)).toList(growable: false),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'client_name': clientName,
      'client_version': clientVersion,
      'correlation_id': correlationId,
      'protocol_max': protocolMax,
      'protocol_min': protocolMin,
      'request_id': requestId,
      'requested_capabilities': requestedCapabilities.map((item) => item.toJson()).toList(growable: false),
    };
  }
}

class CoreHello {
  const CoreHello({
    required this.application,
    required this.correlationId,
    required this.instanceId,
    required this.readiness,
    required this.requestId,
    required this.selectedProtocol,
    required this.supportedCapabilities,
  });

  final ApplicationInfo application;
  final CorrelationId correlationId;
  final InstanceId instanceId;
  final ReadinessReport readiness;
  final RequestId requestId;
  final int selectedProtocol;
  final List<TransportCapability> supportedCapabilities;

  factory CoreHello.fromJson(Map<String, dynamic> json) {
    return CoreHello(
      application: ApplicationInfo.fromJson(_contractMap(json['application'], 'CoreHello.application')),
      correlationId: _contractString(json['correlation_id'], 'CoreHello.correlation_id'),
      instanceId: _contractString(json['instance_id'], 'CoreHello.instance_id'),
      readiness: ReadinessReport.fromJson(_contractMap(json['readiness'], 'CoreHello.readiness')),
      requestId: _contractString(json['request_id'], 'CoreHello.request_id'),
      selectedProtocol: _contractInt(json['selected_protocol'], 'CoreHello.selected_protocol'),
      supportedCapabilities: _contractList(json['supported_capabilities'], 'CoreHello.supported_capabilities').map((item) => TransportCapability.fromJson(item)).toList(growable: false),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'application': application.toJson(),
      'correlation_id': correlationId,
      'instance_id': instanceId,
      'readiness': readiness.toJson(),
      'request_id': requestId,
      'selected_protocol': selectedProtocol,
      'supported_capabilities': supportedCapabilities.map((item) => item.toJson()).toList(growable: false),
    };
  }
}

typedef CorrelationId = String;

class CpuEvidence {
  const CpuEvidence({
    required this.logicalCoreCount,
    required this.name,
    required this.physicalCoreCount,
    required this.vendor,
  });

  final U32Evidence logicalCoreCount;
  final StringEvidence name;
  final U32Evidence physicalCoreCount;
  final StringEvidence vendor;

  factory CpuEvidence.fromJson(Map<String, dynamic> json) {
    return CpuEvidence(
      logicalCoreCount: U32Evidence.fromJson(_contractMap(json['logical_core_count'], 'CpuEvidence.logical_core_count')),
      name: StringEvidence.fromJson(_contractMap(json['name'], 'CpuEvidence.name')),
      physicalCoreCount: U32Evidence.fromJson(_contractMap(json['physical_core_count'], 'CpuEvidence.physical_core_count')),
      vendor: StringEvidence.fromJson(_contractMap(json['vendor'], 'CpuEvidence.vendor')),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'logical_core_count': logicalCoreCount.toJson(),
      'name': name.toJson(),
      'physical_core_count': physicalCoreCount.toJson(),
      'vendor': vendor.toJson(),
    };
  }
}

enum ErrorCategory {
  invalidInput('invalid_input'),
  permissionDenied('permission_denied'),
  notSupported('not_supported'),
  unavailable('unavailable'),
  conflict('conflict'),
  resourceExhausted('resource_exhausted'),
  cancelled('cancelled'),
  timedOut('timed_out'),
  integrityFailure('integrity_failure'),
  incompatibleVersion('incompatible_version'),
  degraded('degraded'),
  internal('internal'),
  unknown('unknown'),
  ;

  const ErrorCategory(this.wireValue);

  final String wireValue;

  static ErrorCategory fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

enum EvidenceAvailability {
  available('available'),
  permissionDenied('permission_denied'),
  timedOut('timed_out'),
  unsupported('unsupported'),
  notPresent('not_present'),
  notReliable('not_reliable'),
  unknown('unknown'),
  ;

  const EvidenceAvailability(this.wireValue);

  final String wireValue;

  static EvidenceAvailability fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

enum EvidenceConfidence {
  high('high'),
  medium('medium'),
  low('low'),
  unknown('unknown'),
  ;

  const EvidenceConfidence(this.wireValue);

  final String wireValue;

  static EvidenceConfidence fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class EvidenceMetadata {
  const EvidenceMetadata({
    required this.availability,
    required this.confidence,
    required this.reason,
    required this.reasonCode,
    required this.source,
  });

  final EvidenceAvailability availability;
  final EvidenceConfidence confidence;
  final String? reason;
  final UnknownReasonCode? reasonCode;
  final EvidenceSource source;

  factory EvidenceMetadata.fromJson(Map<String, dynamic> json) {
    return EvidenceMetadata(
      availability: EvidenceAvailability.fromJson(json['availability']),
      confidence: EvidenceConfidence.fromJson(json['confidence']),
      reason: json['reason'] == null ? null : _contractString(json['reason'], 'EvidenceMetadata.reason'),
      reasonCode: json['reason_code'] == null ? null : UnknownReasonCode.fromJson(json['reason_code']),
      source: EvidenceSource.fromJson(json['source']),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'availability': availability.toJson(),
      'confidence': confidence.toJson(),
      'reason': reason,
      'reason_code': reasonCode?.toJson(),
      'source': source.toJson(),
    };
  }
}

enum EvidenceSource {
  windowsCim('windows_cim'),
  windowsEnvironment('windows_environment'),
  derived('derived'),
  unknown('unknown'),
  ;

  const EvidenceSource(this.wireValue);

  final String wireValue;

  static EvidenceSource fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class GpuCollectionEvidence {
  const GpuCollectionEvidence({
    required this.devices,
    required this.metadata,
  });

  final List<GpuEvidence> devices;
  final EvidenceMetadata metadata;

  factory GpuCollectionEvidence.fromJson(Map<String, dynamic> json) {
    return GpuCollectionEvidence(
      devices: _contractList(json['devices'], 'GpuCollectionEvidence.devices').map((item) => GpuEvidence.fromJson(_contractMap(item, 'GpuCollectionEvidence.devices[]'))).toList(growable: false),
      metadata: EvidenceMetadata.fromJson(_contractMap(json['metadata'], 'GpuCollectionEvidence.metadata')),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'devices': devices.map((item) => item.toJson()).toList(growable: false),
      'metadata': metadata.toJson(),
    };
  }
}

class GpuEvidence {
  const GpuEvidence({
    required this.dedicatedMemoryBytes,
    required this.name,
    required this.sharedMemoryBytes,
    required this.vendor,
  });

  final U64Evidence dedicatedMemoryBytes;
  final StringEvidence name;
  final U64Evidence sharedMemoryBytes;
  final StringEvidence vendor;

  factory GpuEvidence.fromJson(Map<String, dynamic> json) {
    return GpuEvidence(
      dedicatedMemoryBytes: U64Evidence.fromJson(_contractMap(json['dedicated_memory_bytes'], 'GpuEvidence.dedicated_memory_bytes')),
      name: StringEvidence.fromJson(_contractMap(json['name'], 'GpuEvidence.name')),
      sharedMemoryBytes: U64Evidence.fromJson(_contractMap(json['shared_memory_bytes'], 'GpuEvidence.shared_memory_bytes')),
      vendor: StringEvidence.fromJson(_contractMap(json['vendor'], 'GpuEvidence.vendor')),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'dedicated_memory_bytes': dedicatedMemoryBytes.toJson(),
      'name': name.toJson(),
      'shared_memory_bytes': sharedMemoryBytes.toJson(),
      'vendor': vendor.toJson(),
    };
  }
}

class HardwareScanEvent {
  const HardwareScanEvent({
    required this.correlationId,
    required this.error,
    required this.kind,
    required this.message,
    required this.operationId,
    required this.profile,
    required this.schemaVersion,
    required this.sequence,
    required this.terminalState,
    required this.timestampUnixMs,
  });

  final CorrelationId correlationId;
  final SafeErrorPayload? error;
  final HardwareScanEventKind kind;
  final String? message;
  final OperationId operationId;
  final MachineProfile? profile;
  final int schemaVersion;
  final int sequence;
  final HardwareScanTerminalState? terminalState;
  final int timestampUnixMs;

  factory HardwareScanEvent.fromJson(Map<String, dynamic> json) {
    return HardwareScanEvent(
      correlationId: _contractString(json['correlation_id'], 'HardwareScanEvent.correlation_id'),
      error: json['error'] == null ? null : SafeErrorPayload.fromJson(_contractMap(json['error'], 'HardwareScanEvent.error')),
      kind: HardwareScanEventKind.fromJson(json['kind']),
      message: json['message'] == null ? null : _contractString(json['message'], 'HardwareScanEvent.message'),
      operationId: _contractString(json['operation_id'], 'HardwareScanEvent.operation_id'),
      profile: json['profile'] == null ? null : MachineProfile.fromJson(_contractMap(json['profile'], 'HardwareScanEvent.profile')),
      schemaVersion: _contractInt(json['schema_version'], 'HardwareScanEvent.schema_version'),
      sequence: _contractInt(json['sequence'], 'HardwareScanEvent.sequence'),
      terminalState: json['terminal_state'] == null ? null : HardwareScanTerminalState.fromJson(json['terminal_state']),
      timestampUnixMs: _contractInt(json['timestamp_unix_ms'], 'HardwareScanEvent.timestamp_unix_ms'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'error': error?.toJson(),
      'kind': kind.toJson(),
      'message': message,
      'operation_id': operationId,
      'profile': profile?.toJson(),
      'schema_version': schemaVersion,
      'sequence': sequence,
      'terminal_state': terminalState?.toJson(),
      'timestamp_unix_ms': timestampUnixMs,
    };
  }
}

enum HardwareScanEventKind {
  started('started'),
  collecting('collecting'),
  completed('completed'),
  cancelled('cancelled'),
  timedOut('timed_out'),
  failed('failed'),
  unknown('unknown'),
  ;

  const HardwareScanEventKind(this.wireValue);

  final String wireValue;

  static HardwareScanEventKind fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class HardwareScanStartRequest {
  const HardwareScanStartRequest({
    required this.correlationId,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final RequestId requestId;

  factory HardwareScanStartRequest.fromJson(Map<String, dynamic> json) {
    return HardwareScanStartRequest(
      correlationId: _contractString(json['correlation_id'], 'HardwareScanStartRequest.correlation_id'),
      requestId: _contractString(json['request_id'], 'HardwareScanStartRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'request_id': requestId,
    };
  }
}

class HardwareScanStartResponse {
  const HardwareScanStartResponse({
    required this.correlationId,
    required this.operationId,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final OperationId operationId;
  final RequestId requestId;

  factory HardwareScanStartResponse.fromJson(Map<String, dynamic> json) {
    return HardwareScanStartResponse(
      correlationId: _contractString(json['correlation_id'], 'HardwareScanStartResponse.correlation_id'),
      operationId: _contractString(json['operation_id'], 'HardwareScanStartResponse.operation_id'),
      requestId: _contractString(json['request_id'], 'HardwareScanStartResponse.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'operation_id': operationId,
      'request_id': requestId,
    };
  }
}

enum HardwareScanTerminalState {
  completed('completed'),
  partial('partial'),
  cancelled('cancelled'),
  timedOut('timed_out'),
  failed('failed'),
  unknown('unknown'),
  ;

  const HardwareScanTerminalState(this.wireValue);

  final String wireValue;

  static HardwareScanTerminalState fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class HealthRequest {
  const HealthRequest({
    required this.correlationId,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final RequestId requestId;

  factory HealthRequest.fromJson(Map<String, dynamic> json) {
    return HealthRequest(
      correlationId: _contractString(json['correlation_id'], 'HealthRequest.correlation_id'),
      requestId: _contractString(json['request_id'], 'HealthRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'request_id': requestId,
    };
  }
}

class HealthResponse {
  const HealthResponse({
    required this.correlationId,
    required this.requestId,
    required this.status,
  });

  final CorrelationId correlationId;
  final RequestId requestId;
  final PlatformStatus status;

  factory HealthResponse.fromJson(Map<String, dynamic> json) {
    return HealthResponse(
      correlationId: _contractString(json['correlation_id'], 'HealthResponse.correlation_id'),
      requestId: _contractString(json['request_id'], 'HealthResponse.request_id'),
      status: PlatformStatus.fromJson(_contractMap(json['status'], 'HealthResponse.status')),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'request_id': requestId,
      'status': status.toJson(),
    };
  }
}

typedef InstanceId = String;

enum MachineArchitecture {
  x8664('x86_64'),
  arm64('arm64'),
  x86('x86'),
  unknown('unknown'),
  ;

  const MachineArchitecture(this.wireValue);

  final String wireValue;

  static MachineArchitecture fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class MachineProfile {
  const MachineProfile({
    required this.acceleration,
    required this.completeness,
    required this.correlationId,
    required this.cpu,
    required this.gpus,
    required this.operatingSystem,
    required this.physicalMemory,
    required this.scanId,
    required this.scannedAtUnixMs,
    required this.schemaVersion,
    required this.storage,
  });

  final List<AccelerationEvidence> acceleration;
  final MachineProfileCompleteness completeness;
  final CorrelationId correlationId;
  final CpuEvidence cpu;
  final GpuCollectionEvidence gpus;
  final OperatingSystemEvidence operatingSystem;
  final PhysicalMemoryEvidence physicalMemory;
  final OperationId scanId;
  final int scannedAtUnixMs;
  final int schemaVersion;
  final StorageEvidence storage;

  factory MachineProfile.fromJson(Map<String, dynamic> json) {
    return MachineProfile(
      acceleration: _contractList(json['acceleration'], 'MachineProfile.acceleration').map((item) => AccelerationEvidence.fromJson(_contractMap(item, 'MachineProfile.acceleration[]'))).toList(growable: false),
      completeness: MachineProfileCompleteness.fromJson(json['completeness']),
      correlationId: _contractString(json['correlation_id'], 'MachineProfile.correlation_id'),
      cpu: CpuEvidence.fromJson(_contractMap(json['cpu'], 'MachineProfile.cpu')),
      gpus: GpuCollectionEvidence.fromJson(_contractMap(json['gpus'], 'MachineProfile.gpus')),
      operatingSystem: OperatingSystemEvidence.fromJson(_contractMap(json['operating_system'], 'MachineProfile.operating_system')),
      physicalMemory: PhysicalMemoryEvidence.fromJson(_contractMap(json['physical_memory'], 'MachineProfile.physical_memory')),
      scanId: _contractString(json['scan_id'], 'MachineProfile.scan_id'),
      scannedAtUnixMs: _contractInt(json['scanned_at_unix_ms'], 'MachineProfile.scanned_at_unix_ms'),
      schemaVersion: _contractInt(json['schema_version'], 'MachineProfile.schema_version'),
      storage: StorageEvidence.fromJson(_contractMap(json['storage'], 'MachineProfile.storage')),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'acceleration': acceleration.map((item) => item.toJson()).toList(growable: false),
      'completeness': completeness.toJson(),
      'correlation_id': correlationId,
      'cpu': cpu.toJson(),
      'gpus': gpus.toJson(),
      'operating_system': operatingSystem.toJson(),
      'physical_memory': physicalMemory.toJson(),
      'scan_id': scanId,
      'scanned_at_unix_ms': scannedAtUnixMs,
      'schema_version': schemaVersion,
      'storage': storage.toJson(),
    };
  }
}

enum MachineProfileCompleteness {
  complete('complete'),
  partial('partial'),
  unknown('unknown'),
  ;

  const MachineProfileCompleteness(this.wireValue);

  final String wireValue;

  static MachineProfileCompleteness fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class OperatingSystemEvidence {
  const OperatingSystemEvidence({
    required this.architecture,
    required this.build,
    required this.name,
    required this.version,
  });

  final ArchitectureEvidence architecture;
  final StringEvidence build;
  final StringEvidence name;
  final StringEvidence version;

  factory OperatingSystemEvidence.fromJson(Map<String, dynamic> json) {
    return OperatingSystemEvidence(
      architecture: ArchitectureEvidence.fromJson(_contractMap(json['architecture'], 'OperatingSystemEvidence.architecture')),
      build: StringEvidence.fromJson(_contractMap(json['build'], 'OperatingSystemEvidence.build')),
      name: StringEvidence.fromJson(_contractMap(json['name'], 'OperatingSystemEvidence.name')),
      version: StringEvidence.fromJson(_contractMap(json['version'], 'OperatingSystemEvidence.version')),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'architecture': architecture.toJson(),
      'build': build.toJson(),
      'name': name.toJson(),
      'version': version.toJson(),
    };
  }
}

typedef OperationId = String;

class PhysicalMemoryEvidence {
  const PhysicalMemoryEvidence({
    required this.availableBytes,
    required this.totalBytes,
  });

  final U64Evidence availableBytes;
  final U64Evidence totalBytes;

  factory PhysicalMemoryEvidence.fromJson(Map<String, dynamic> json) {
    return PhysicalMemoryEvidence(
      availableBytes: U64Evidence.fromJson(_contractMap(json['available_bytes'], 'PhysicalMemoryEvidence.available_bytes')),
      totalBytes: U64Evidence.fromJson(_contractMap(json['total_bytes'], 'PhysicalMemoryEvidence.total_bytes')),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'available_bytes': availableBytes.toJson(),
      'total_bytes': totalBytes.toJson(),
    };
  }
}

class PlatformStatus {
  const PlatformStatus({
    required this.application,
    required this.readiness,
  });

  final ApplicationInfo application;
  final ReadinessReport readiness;

  factory PlatformStatus.fromJson(Map<String, dynamic> json) {
    return PlatformStatus(
      application: ApplicationInfo.fromJson(_contractMap(json['application'], 'PlatformStatus.application')),
      readiness: ReadinessReport.fromJson(_contractMap(json['readiness'], 'PlatformStatus.readiness')),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'application': application.toJson(),
      'readiness': readiness.toJson(),
    };
  }
}

class ReadinessReport {
  const ReadinessReport({
    required this.services,
    required this.status,
    required this.summary,
  });

  final List<ServiceHealth> services;
  final ReadinessStatus status;
  final String summary;

  factory ReadinessReport.fromJson(Map<String, dynamic> json) {
    return ReadinessReport(
      services: _contractList(json['services'], 'ReadinessReport.services').map((item) => ServiceHealth.fromJson(_contractMap(item, 'ReadinessReport.services[]'))).toList(growable: false),
      status: ReadinessStatus.fromJson(json['status']),
      summary: _contractString(json['summary'], 'ReadinessReport.summary'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'services': services.map((item) => item.toJson()).toList(growable: false),
      'status': status.toJson(),
      'summary': summary,
    };
  }
}

enum ReadinessStatus {
  ready('ready'),
  degraded('degraded'),
  unavailable('unavailable'),
  failed('failed'),
  unknown('unknown'),
  ;

  const ReadinessStatus(this.wireValue);

  final String wireValue;

  static ReadinessStatus fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

enum RecoveryAction {
  retry('retry'),
  restart('restart'),
  checkPrerequisites('check_prerequisites'),
  contactSupport('contact_support'),
  noAction('no_action'),
  unknown('unknown'),
  ;

  const RecoveryAction(this.wireValue);

  final String wireValue;

  static RecoveryAction fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class RecoveryGuidance {
  const RecoveryGuidance({
    required this.action,
    required this.message,
  });

  final RecoveryAction action;
  final String message;

  factory RecoveryGuidance.fromJson(Map<String, dynamic> json) {
    return RecoveryGuidance(
      action: RecoveryAction.fromJson(json['action']),
      message: _contractString(json['message'], 'RecoveryGuidance.message'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'action': action.toJson(),
      'message': message,
    };
  }
}

typedef RequestId = String;

class SafeErrorPayload {
  const SafeErrorPayload({
    required this.category,
    required this.code,
    required this.correlationId,
    required this.message,
    required this.recovery,
    required this.requestId,
  });

  final ErrorCategory category;
  final String code;
  final CorrelationId correlationId;
  final String message;
  final RecoveryGuidance recovery;
  final RequestId requestId;

  factory SafeErrorPayload.fromJson(Map<String, dynamic> json) {
    return SafeErrorPayload(
      category: ErrorCategory.fromJson(json['category']),
      code: _contractString(json['code'], 'SafeErrorPayload.code'),
      correlationId: _contractString(json['correlation_id'], 'SafeErrorPayload.correlation_id'),
      message: _contractString(json['message'], 'SafeErrorPayload.message'),
      recovery: RecoveryGuidance.fromJson(_contractMap(json['recovery'], 'SafeErrorPayload.recovery')),
      requestId: _contractString(json['request_id'], 'SafeErrorPayload.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'category': category.toJson(),
      'code': code,
      'correlation_id': correlationId,
      'message': message,
      'recovery': recovery.toJson(),
      'request_id': requestId,
    };
  }
}

class ServiceHealth {
  const ServiceHealth({
    required this.displayName,
    required this.message,
    required this.requirement,
    required this.serviceId,
    required this.status,
  });

  final String displayName;
  final String? message;
  final ServiceRequirement requirement;
  final String serviceId;
  final ServiceHealthStatus status;

  factory ServiceHealth.fromJson(Map<String, dynamic> json) {
    return ServiceHealth(
      displayName: _contractString(json['display_name'], 'ServiceHealth.display_name'),
      message: json['message'] == null ? null : _contractString(json['message'], 'ServiceHealth.message'),
      requirement: ServiceRequirement.fromJson(json['requirement']),
      serviceId: _contractString(json['service_id'], 'ServiceHealth.service_id'),
      status: ServiceHealthStatus.fromJson(json['status']),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'display_name': displayName,
      'message': message,
      'requirement': requirement.toJson(),
      'service_id': serviceId,
      'status': status.toJson(),
    };
  }
}

enum ServiceHealthStatus {
  healthy('healthy'),
  degraded('degraded'),
  unavailable('unavailable'),
  failed('failed'),
  unknown('unknown'),
  ;

  const ServiceHealthStatus(this.wireValue);

  final String wireValue;

  static ServiceHealthStatus fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

enum ServiceRequirement {
  mandatory('mandatory'),
  optional('optional'),
  unknown('unknown'),
  ;

  const ServiceRequirement(this.wireValue);

  final String wireValue;

  static ServiceRequirement fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class ShutdownRequest {
  const ShutdownRequest({
    required this.correlationId,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final RequestId requestId;

  factory ShutdownRequest.fromJson(Map<String, dynamic> json) {
    return ShutdownRequest(
      correlationId: _contractString(json['correlation_id'], 'ShutdownRequest.correlation_id'),
      requestId: _contractString(json['request_id'], 'ShutdownRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'request_id': requestId,
    };
  }
}

class ShutdownResponse {
  const ShutdownResponse({
    required this.accepted,
    required this.correlationId,
    required this.requestId,
  });

  final bool accepted;
  final CorrelationId correlationId;
  final RequestId requestId;

  factory ShutdownResponse.fromJson(Map<String, dynamic> json) {
    return ShutdownResponse(
      accepted: _contractBool(json['accepted'], 'ShutdownResponse.accepted'),
      correlationId: _contractString(json['correlation_id'], 'ShutdownResponse.correlation_id'),
      requestId: _contractString(json['request_id'], 'ShutdownResponse.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'accepted': accepted,
      'correlation_id': correlationId,
      'request_id': requestId,
    };
  }
}

class StorageEvidence {
  const StorageEvidence({
    required this.capacityBytes,
    required this.filesystem,
    required this.freeBytes,
    required this.location,
    required this.mediaKind,
  });

  final U64Evidence capacityBytes;
  final StringEvidence filesystem;
  final U64Evidence freeBytes;
  final StorageLocation location;
  final StorageMediaEvidence mediaKind;

  factory StorageEvidence.fromJson(Map<String, dynamic> json) {
    return StorageEvidence(
      capacityBytes: U64Evidence.fromJson(_contractMap(json['capacity_bytes'], 'StorageEvidence.capacity_bytes')),
      filesystem: StringEvidence.fromJson(_contractMap(json['filesystem'], 'StorageEvidence.filesystem')),
      freeBytes: U64Evidence.fromJson(_contractMap(json['free_bytes'], 'StorageEvidence.free_bytes')),
      location: StorageLocation.fromJson(json['location']),
      mediaKind: StorageMediaEvidence.fromJson(_contractMap(json['media_kind'], 'StorageEvidence.media_kind')),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'capacity_bytes': capacityBytes.toJson(),
      'filesystem': filesystem.toJson(),
      'free_bytes': freeBytes.toJson(),
      'location': location.toJson(),
      'media_kind': mediaKind.toJson(),
    };
  }
}

enum StorageLocation {
  applicationData('application_data'),
  unknown('unknown'),
  ;

  const StorageLocation(this.wireValue);

  final String wireValue;

  static StorageLocation fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class StorageMediaEvidence {
  const StorageMediaEvidence({
    required this.metadata,
    required this.value,
  });

  final EvidenceMetadata metadata;
  final StorageMediaKind? value;

  factory StorageMediaEvidence.fromJson(Map<String, dynamic> json) {
    return StorageMediaEvidence(
      metadata: EvidenceMetadata.fromJson(_contractMap(json['metadata'], 'StorageMediaEvidence.metadata')),
      value: json['value'] == null ? null : StorageMediaKind.fromJson(json['value']),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'metadata': metadata.toJson(),
      'value': value?.toJson(),
    };
  }
}

enum StorageMediaKind {
  fixed('fixed'),
  removable('removable'),
  network('network'),
  optical('optical'),
  ramDisk('ram_disk'),
  unknown('unknown'),
  ;

  const StorageMediaKind(this.wireValue);

  final String wireValue;

  static StorageMediaKind fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class StringEvidence {
  const StringEvidence({
    required this.metadata,
    required this.value,
  });

  final EvidenceMetadata metadata;
  final String? value;

  factory StringEvidence.fromJson(Map<String, dynamic> json) {
    return StringEvidence(
      metadata: EvidenceMetadata.fromJson(_contractMap(json['metadata'], 'StringEvidence.metadata')),
      value: json['value'] == null ? null : _contractString(json['value'], 'StringEvidence.value'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'metadata': metadata.toJson(),
      'value': value,
    };
  }
}

typedef SupervisionNonce = String;

class TestOperationEvent {
  const TestOperationEvent({
    required this.correlationId,
    required this.kind,
    required this.message,
    required this.operationId,
    required this.schemaVersion,
    required this.sequence,
    required this.terminalState,
    required this.timestampUnixMs,
  });

  final CorrelationId correlationId;
  final TestOperationEventKind kind;
  final String? message;
  final OperationId operationId;
  final int schemaVersion;
  final int sequence;
  final TestOperationTerminalState? terminalState;
  final int timestampUnixMs;

  factory TestOperationEvent.fromJson(Map<String, dynamic> json) {
    return TestOperationEvent(
      correlationId: _contractString(json['correlation_id'], 'TestOperationEvent.correlation_id'),
      kind: TestOperationEventKind.fromJson(json['kind']),
      message: json['message'] == null ? null : _contractString(json['message'], 'TestOperationEvent.message'),
      operationId: _contractString(json['operation_id'], 'TestOperationEvent.operation_id'),
      schemaVersion: _contractInt(json['schema_version'], 'TestOperationEvent.schema_version'),
      sequence: _contractInt(json['sequence'], 'TestOperationEvent.sequence'),
      terminalState: json['terminal_state'] == null ? null : TestOperationTerminalState.fromJson(json['terminal_state']),
      timestampUnixMs: _contractInt(json['timestamp_unix_ms'], 'TestOperationEvent.timestamp_unix_ms'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'kind': kind.toJson(),
      'message': message,
      'operation_id': operationId,
      'schema_version': schemaVersion,
      'sequence': sequence,
      'terminal_state': terminalState?.toJson(),
      'timestamp_unix_ms': timestampUnixMs,
    };
  }
}

enum TestOperationEventKind {
  started('started'),
  progress('progress'),
  completed('completed'),
  cancelled('cancelled'),
  failed('failed'),
  timedOut('timed_out'),
  unknown('unknown'),
  ;

  const TestOperationEventKind(this.wireValue);

  final String wireValue;

  static TestOperationEventKind fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class TestOperationStartRequest {
  const TestOperationStartRequest({
    required this.correlationId,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final RequestId requestId;

  factory TestOperationStartRequest.fromJson(Map<String, dynamic> json) {
    return TestOperationStartRequest(
      correlationId: _contractString(json['correlation_id'], 'TestOperationStartRequest.correlation_id'),
      requestId: _contractString(json['request_id'], 'TestOperationStartRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'request_id': requestId,
    };
  }
}

class TestOperationStartResponse {
  const TestOperationStartResponse({
    required this.correlationId,
    required this.operationId,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final OperationId operationId;
  final RequestId requestId;

  factory TestOperationStartResponse.fromJson(Map<String, dynamic> json) {
    return TestOperationStartResponse(
      correlationId: _contractString(json['correlation_id'], 'TestOperationStartResponse.correlation_id'),
      operationId: _contractString(json['operation_id'], 'TestOperationStartResponse.operation_id'),
      requestId: _contractString(json['request_id'], 'TestOperationStartResponse.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'operation_id': operationId,
      'request_id': requestId,
    };
  }
}

enum TestOperationTerminalState {
  completed('completed'),
  cancelled('cancelled'),
  failed('failed'),
  timedOut('timed_out'),
  unknown('unknown'),
  ;

  const TestOperationTerminalState(this.wireValue);

  final String wireValue;

  static TestOperationTerminalState fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

enum TransportCapability {
  health('health'),
  testOperationEvents('test_operation_events'),
  cancellation('cancellation'),
  hardwareScan('hardware_scan'),
  shutdown('shutdown'),
  unknown('unknown'),
  ;

  const TransportCapability(this.wireValue);

  final String wireValue;

  static TransportCapability fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class U32Evidence {
  const U32Evidence({
    required this.metadata,
    required this.value,
  });

  final EvidenceMetadata metadata;
  final int? value;

  factory U32Evidence.fromJson(Map<String, dynamic> json) {
    return U32Evidence(
      metadata: EvidenceMetadata.fromJson(_contractMap(json['metadata'], 'U32Evidence.metadata')),
      value: json['value'] == null ? null : _contractInt(json['value'], 'U32Evidence.value'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'metadata': metadata.toJson(),
      'value': value,
    };
  }
}

class U64Evidence {
  const U64Evidence({
    required this.metadata,
    required this.value,
  });

  final EvidenceMetadata metadata;
  final int? value;

  factory U64Evidence.fromJson(Map<String, dynamic> json) {
    return U64Evidence(
      metadata: EvidenceMetadata.fromJson(_contractMap(json['metadata'], 'U64Evidence.metadata')),
      value: json['value'] == null ? null : _contractInt(json['value'], 'U64Evidence.value'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'metadata': metadata.toJson(),
      'value': value,
    };
  }
}

enum UnknownReasonCode {
  notReported('not_reported'),
  accessDenied('access_denied'),
  deadlineExceeded('deadline_exceeded'),
  providerUnsupported('provider_unsupported'),
  deviceAbsent('device_absent'),
  sourceUnreliable('source_unreliable'),
  invalidProviderData('invalid_provider_data'),
  unknown('unknown'),
  ;

  const UnknownReasonCode(this.wireValue);

  final String wireValue;

  static UnknownReasonCode fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

Map<String, dynamic> _contractMap(Object? value, String path) {
  if (value is Map<String, dynamic>) {
    return value;
  }
  throw FormatException('$path must be a JSON object.');
}

List<dynamic> _contractList(Object? value, String path) {
  if (value is List<dynamic>) {
    return value;
  }
  throw FormatException('$path must be a JSON array.');
}

String _contractString(Object? value, String path) {
  if (value is String) {
    return value;
  }
  throw FormatException('$path must be a string.');
}

int _contractInt(Object? value, String path) {
  if (value is int) {
    return value;
  }
  throw FormatException('$path must be an integer.');
}

bool _contractBool(Object? value, String path) {
  if (value is bool) {
    return value;
  }
  throw FormatException('$path must be a boolean.');
}
