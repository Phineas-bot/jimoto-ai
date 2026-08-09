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

class CandidateModel {
  const CandidateModel({
    required this.catalogueId,
    required this.displayName,
    required this.family,
    required this.licenceSpdx,
    required this.provenanceUrl,
    required this.sizeClass,
    required this.workloadTiers,
  });

  final CandidateModelId catalogueId;
  final String displayName;
  final String family;
  final String licenceSpdx;
  final String provenanceUrl;
  final ModelSizeClass sizeClass;
  final List<WorkloadTier> workloadTiers;

  factory CandidateModel.fromJson(Map<String, dynamic> json) {
    return CandidateModel(
      catalogueId: _contractString(json['catalogue_id'], 'CandidateModel.catalogue_id'),
      displayName: _contractString(json['display_name'], 'CandidateModel.display_name'),
      family: _contractString(json['family'], 'CandidateModel.family'),
      licenceSpdx: _contractString(json['licence_spdx'], 'CandidateModel.licence_spdx'),
      provenanceUrl: _contractString(json['provenance_url'], 'CandidateModel.provenance_url'),
      sizeClass: ModelSizeClass.fromJson(json['size_class']),
      workloadTiers: _contractList(json['workload_tiers'], 'CandidateModel.workload_tiers').map((item) => WorkloadTier.fromJson(item)).toList(growable: false),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'catalogue_id': catalogueId,
      'display_name': displayName,
      'family': family,
      'licence_spdx': licenceSpdx,
      'provenance_url': provenanceUrl,
      'size_class': sizeClass.toJson(),
      'workload_tiers': workloadTiers.map((item) => item.toJson()).toList(growable: false),
    };
  }
}

typedef CandidateModelId = String;

class CandidateRuntime {
  const CandidateRuntime({
    required this.catalogueId,
    required this.displayName,
    required this.optionalAccelerations,
    required this.supportedArchitectures,
    required this.supportsCpuOnly,
  });

  final CandidateRuntimeId catalogueId;
  final String displayName;
  final List<AccelerationKind> optionalAccelerations;
  final List<MachineArchitecture> supportedArchitectures;
  final bool supportsCpuOnly;

  factory CandidateRuntime.fromJson(Map<String, dynamic> json) {
    return CandidateRuntime(
      catalogueId: _contractString(json['catalogue_id'], 'CandidateRuntime.catalogue_id'),
      displayName: _contractString(json['display_name'], 'CandidateRuntime.display_name'),
      optionalAccelerations: _contractList(json['optional_accelerations'], 'CandidateRuntime.optional_accelerations').map((item) => AccelerationKind.fromJson(item)).toList(growable: false),
      supportedArchitectures: _contractList(json['supported_architectures'], 'CandidateRuntime.supported_architectures').map((item) => MachineArchitecture.fromJson(item)).toList(growable: false),
      supportsCpuOnly: _contractBool(json['supports_cpu_only'], 'CandidateRuntime.supports_cpu_only'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'catalogue_id': catalogueId,
      'display_name': displayName,
      'optional_accelerations': optionalAccelerations.map((item) => item.toJson()).toList(growable: false),
      'supported_architectures': supportedArchitectures.map((item) => item.toJson()).toList(growable: false),
      'supports_cpu_only': supportsCpuOnly,
    };
  }
}

typedef CandidateRuntimeId = String;

class CapabilityReport {
  const CapabilityReport({
    required this.catalogueVersion,
    required this.confidence,
    required this.fallbackPlan,
    required this.generatedFromScanUnixMs,
    required this.machineProfileSchemaVersion,
    required this.noPlan,
    required this.optionalLargerPlan,
    required this.preferences,
    required this.reasons,
    required this.recommendedPlan,
    required this.ruleSetVersion,
    required this.schemaVersion,
    required this.status,
    required this.warnings,
  });

  final CatalogueVersion catalogueVersion;
  final ConfidenceLevel confidence;
  final RecommendationPlan? fallbackPlan;
  final int generatedFromScanUnixMs;
  final int machineProfileSchemaVersion;
  final NoPlanResult? noPlan;
  final RecommendationPlan? optionalLargerPlan;
  final UserPreferenceProfile preferences;
  final List<RecommendationReason> reasons;
  final RecommendationPlan? recommendedPlan;
  final RuleSetVersion ruleSetVersion;
  final int schemaVersion;
  final CapabilityReportStatus status;
  final List<RecommendationWarning> warnings;

  factory CapabilityReport.fromJson(Map<String, dynamic> json) {
    return CapabilityReport(
      catalogueVersion: _contractString(json['catalogue_version'], 'CapabilityReport.catalogue_version'),
      confidence: ConfidenceLevel.fromJson(json['confidence']),
      fallbackPlan: json['fallback_plan'] == null ? null : RecommendationPlan.fromJson(_contractMap(json['fallback_plan'], 'CapabilityReport.fallback_plan')),
      generatedFromScanUnixMs: _contractInt(json['generated_from_scan_unix_ms'], 'CapabilityReport.generated_from_scan_unix_ms'),
      machineProfileSchemaVersion: _contractInt(json['machine_profile_schema_version'], 'CapabilityReport.machine_profile_schema_version'),
      noPlan: json['no_plan'] == null ? null : NoPlanResult.fromJson(_contractMap(json['no_plan'], 'CapabilityReport.no_plan')),
      optionalLargerPlan: json['optional_larger_plan'] == null ? null : RecommendationPlan.fromJson(_contractMap(json['optional_larger_plan'], 'CapabilityReport.optional_larger_plan')),
      preferences: UserPreferenceProfile.fromJson(_contractMap(json['preferences'], 'CapabilityReport.preferences')),
      reasons: _contractList(json['reasons'], 'CapabilityReport.reasons').map((item) => RecommendationReason.fromJson(_contractMap(item, 'CapabilityReport.reasons[]'))).toList(growable: false),
      recommendedPlan: json['recommended_plan'] == null ? null : RecommendationPlan.fromJson(_contractMap(json['recommended_plan'], 'CapabilityReport.recommended_plan')),
      ruleSetVersion: _contractString(json['rule_set_version'], 'CapabilityReport.rule_set_version'),
      schemaVersion: _contractInt(json['schema_version'], 'CapabilityReport.schema_version'),
      status: CapabilityReportStatus.fromJson(json['status']),
      warnings: _contractList(json['warnings'], 'CapabilityReport.warnings').map((item) => RecommendationWarning.fromJson(_contractMap(item, 'CapabilityReport.warnings[]'))).toList(growable: false),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'catalogue_version': catalogueVersion,
      'confidence': confidence.toJson(),
      'fallback_plan': fallbackPlan?.toJson(),
      'generated_from_scan_unix_ms': generatedFromScanUnixMs,
      'machine_profile_schema_version': machineProfileSchemaVersion,
      'no_plan': noPlan?.toJson(),
      'optional_larger_plan': optionalLargerPlan?.toJson(),
      'preferences': preferences.toJson(),
      'reasons': reasons.map((item) => item.toJson()).toList(growable: false),
      'recommended_plan': recommendedPlan?.toJson(),
      'rule_set_version': ruleSetVersion,
      'schema_version': schemaVersion,
      'status': status.toJson(),
      'warnings': warnings.map((item) => item.toJson()).toList(growable: false),
    };
  }
}

enum CapabilityReportStatus {
  plansAvailable('plans_available'),
  noPlan('no_plan'),
  unknown('unknown'),
  ;

  const CapabilityReportStatus(this.wireValue);

  final String wireValue;

  static CapabilityReportStatus fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

typedef CatalogueVersion = String;

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

enum CompatibilityStatus {
  compatible('compatible'),
  incompatible('incompatible'),
  unknown('unknown'),
  ;

  const CompatibilityStatus(this.wireValue);

  final String wireValue;

  static CompatibilityStatus fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

enum ConfidenceLevel {
  high('high'),
  medium('medium'),
  low('low'),
  unknown('unknown'),
  ;

  const ConfidenceLevel(this.wireValue);

  final String wireValue;

  static ConfidenceLevel fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
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

class MemoryEstimate {
  const MemoryEstimate({
    required this.observedAvailableBytes,
    required this.observedTotalBytes,
    required this.requiredBytes,
    required this.safetyMarginBytes,
  });

  final int? observedAvailableBytes;
  final int? observedTotalBytes;
  final int requiredBytes;
  final int safetyMarginBytes;

  factory MemoryEstimate.fromJson(Map<String, dynamic> json) {
    return MemoryEstimate(
      observedAvailableBytes: json['observed_available_bytes'] == null ? null : _contractInt(json['observed_available_bytes'], 'MemoryEstimate.observed_available_bytes'),
      observedTotalBytes: json['observed_total_bytes'] == null ? null : _contractInt(json['observed_total_bytes'], 'MemoryEstimate.observed_total_bytes'),
      requiredBytes: _contractInt(json['required_bytes'], 'MemoryEstimate.required_bytes'),
      safetyMarginBytes: _contractInt(json['safety_margin_bytes'], 'MemoryEstimate.safety_margin_bytes'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'observed_available_bytes': observedAvailableBytes,
      'observed_total_bytes': observedTotalBytes,
      'required_bytes': requiredBytes,
      'safety_margin_bytes': safetyMarginBytes,
    };
  }
}

enum ModelSizeClass {
  compact('compact'),
  standard('standard'),
  large('large'),
  unknown('unknown'),
  ;

  const ModelSizeClass(this.wireValue);

  final String wireValue;

  static ModelSizeClass fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class NoPlanResult {
  const NoPlanResult({
    required this.confidence,
    required this.reasons,
    required this.warnings,
  });

  final ConfidenceLevel confidence;
  final List<RecommendationReason> reasons;
  final List<RecommendationWarning> warnings;

  factory NoPlanResult.fromJson(Map<String, dynamic> json) {
    return NoPlanResult(
      confidence: ConfidenceLevel.fromJson(json['confidence']),
      reasons: _contractList(json['reasons'], 'NoPlanResult.reasons').map((item) => RecommendationReason.fromJson(_contractMap(item, 'NoPlanResult.reasons[]'))).toList(growable: false),
      warnings: _contractList(json['warnings'], 'NoPlanResult.warnings').map((item) => RecommendationWarning.fromJson(_contractMap(item, 'NoPlanResult.warnings[]'))).toList(growable: false),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'confidence': confidence.toJson(),
      'reasons': reasons.map((item) => item.toJson()).toList(growable: false),
      'warnings': warnings.map((item) => item.toJson()).toList(growable: false),
    };
  }
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

enum PlanRole {
  recommended('recommended'),
  fallback('fallback'),
  optionalLarger('optional_larger'),
  unknown('unknown'),
  ;

  const PlanRole(this.wireValue);

  final String wireValue;

  static PlanRole fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
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

enum PreferencePriority {
  balanced('balanced'),
  fastestSetup('fastest_setup'),
  lowestResourceUse('lowest_resource_use'),
  bestQualityWithinSafeLimits('best_quality_within_safe_limits'),
  unknown('unknown'),
  ;

  const PreferencePriority(this.wireValue);

  final String wireValue;

  static PreferencePriority fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
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

class RecommendationPlan {
  const RecommendationPlan({
    required this.catalogueVersion,
    required this.compatibility,
    required this.confidence,
    required this.model,
    required this.reasons,
    required this.resources,
    required this.role,
    required this.ruleSetVersion,
    required this.runtime,
    required this.warnings,
  });

  final CatalogueVersion catalogueVersion;
  final CompatibilityStatus compatibility;
  final ConfidenceLevel confidence;
  final CandidateModel model;
  final List<RecommendationReason> reasons;
  final ResourceEstimate resources;
  final PlanRole role;
  final RuleSetVersion ruleSetVersion;
  final CandidateRuntime runtime;
  final List<RecommendationWarning> warnings;

  factory RecommendationPlan.fromJson(Map<String, dynamic> json) {
    return RecommendationPlan(
      catalogueVersion: _contractString(json['catalogue_version'], 'RecommendationPlan.catalogue_version'),
      compatibility: CompatibilityStatus.fromJson(json['compatibility']),
      confidence: ConfidenceLevel.fromJson(json['confidence']),
      model: CandidateModel.fromJson(_contractMap(json['model'], 'RecommendationPlan.model')),
      reasons: _contractList(json['reasons'], 'RecommendationPlan.reasons').map((item) => RecommendationReason.fromJson(_contractMap(item, 'RecommendationPlan.reasons[]'))).toList(growable: false),
      resources: ResourceEstimate.fromJson(_contractMap(json['resources'], 'RecommendationPlan.resources')),
      role: PlanRole.fromJson(json['role']),
      ruleSetVersion: _contractString(json['rule_set_version'], 'RecommendationPlan.rule_set_version'),
      runtime: CandidateRuntime.fromJson(_contractMap(json['runtime'], 'RecommendationPlan.runtime')),
      warnings: _contractList(json['warnings'], 'RecommendationPlan.warnings').map((item) => RecommendationWarning.fromJson(_contractMap(item, 'RecommendationPlan.warnings[]'))).toList(growable: false),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'catalogue_version': catalogueVersion,
      'compatibility': compatibility.toJson(),
      'confidence': confidence.toJson(),
      'model': model.toJson(),
      'reasons': reasons.map((item) => item.toJson()).toList(growable: false),
      'resources': resources.toJson(),
      'role': role.toJson(),
      'rule_set_version': ruleSetVersion,
      'runtime': runtime.toJson(),
      'warnings': warnings.map((item) => item.toJson()).toList(growable: false),
    };
  }
}

class RecommendationReason {
  const RecommendationReason({
    required this.code,
    required this.message,
  });

  final RecommendationReasonCode code;
  final String message;

  factory RecommendationReason.fromJson(Map<String, dynamic> json) {
    return RecommendationReason(
      code: RecommendationReasonCode.fromJson(json['code']),
      message: _contractString(json['message'], 'RecommendationReason.message'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'code': code.toJson(),
      'message': message,
    };
  }
}

enum RecommendationReasonCode {
  workloadMatch('workload_match'),
  balancedChoice('balanced_choice'),
  fastestSetup('fastest_setup'),
  lowestResourceUse('lowest_resource_use'),
  bestQualityWithinSafeLimits('best_quality_within_safe_limits'),
  safeMemoryMargin('safe_memory_margin'),
  safeStorageMargin('safe_storage_margin'),
  cpuOnlyFeasible('cpu_only_feasible'),
  reliableAccelerationAvailable('reliable_acceleration_available'),
  smallerFallback('smaller_fallback'),
  largerAlternative('larger_alternative'),
  stableCatalogueOrder('stable_catalogue_order'),
  noCompatibleCandidate('no_compatible_candidate'),
  unsupportedArchitecture('unsupported_architecture'),
  insufficientLogicalProcessors('insufficient_logical_processors'),
  insufficientMemory('insufficient_memory'),
  insufficientAvailableMemory('insufficient_available_memory'),
  insufficientStorage('insufficient_storage'),
  criticalEvidenceUnknown('critical_evidence_unknown'),
  unknown('unknown'),
  ;

  const RecommendationReasonCode(this.wireValue);

  final String wireValue;

  static RecommendationReasonCode fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class RecommendationRequest {
  const RecommendationRequest({
    required this.correlationId,
    required this.machineProfile,
    required this.preferences,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final MachineProfile machineProfile;
  final UserPreferenceProfile preferences;
  final RequestId requestId;

  factory RecommendationRequest.fromJson(Map<String, dynamic> json) {
    return RecommendationRequest(
      correlationId: _contractString(json['correlation_id'], 'RecommendationRequest.correlation_id'),
      machineProfile: MachineProfile.fromJson(_contractMap(json['machine_profile'], 'RecommendationRequest.machine_profile')),
      preferences: UserPreferenceProfile.fromJson(_contractMap(json['preferences'], 'RecommendationRequest.preferences')),
      requestId: _contractString(json['request_id'], 'RecommendationRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'machine_profile': machineProfile.toJson(),
      'preferences': preferences.toJson(),
      'request_id': requestId,
    };
  }
}

class RecommendationResponse {
  const RecommendationResponse({
    required this.correlationId,
    required this.report,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final CapabilityReport report;
  final RequestId requestId;

  factory RecommendationResponse.fromJson(Map<String, dynamic> json) {
    return RecommendationResponse(
      correlationId: _contractString(json['correlation_id'], 'RecommendationResponse.correlation_id'),
      report: CapabilityReport.fromJson(_contractMap(json['report'], 'RecommendationResponse.report')),
      requestId: _contractString(json['request_id'], 'RecommendationResponse.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'report': report.toJson(),
      'request_id': requestId,
    };
  }
}

class RecommendationWarning {
  const RecommendationWarning({
    required this.code,
    required this.message,
  });

  final RecommendationWarningCode code;
  final String message;

  factory RecommendationWarning.fromJson(Map<String, dynamic> json) {
    return RecommendationWarning(
      code: RecommendationWarningCode.fromJson(json['code']),
      message: _contractString(json['message'], 'RecommendationWarning.message'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'code': code.toJson(),
      'message': message,
    };
  }
}

enum RecommendationWarningCode {
  partialHardwareEvidence('partial_hardware_evidence'),
  availableMemoryUnknown('available_memory_unknown'),
  vramUnknown('vram_unknown'),
  accelerationUnknown('acceleration_unknown'),
  cpuOnlyMode('cpu_only_mode'),
  removableStorage('removable_storage'),
  limitedFallbackOptions('limited_fallback_options'),
  optionalLargerUnavailable('optional_larger_unavailable'),
  noSafePlan('no_safe_plan'),
  unknown('unknown'),
  ;

  const RecommendationWarningCode(this.wireValue);

  final String wireValue;

  static RecommendationWarningCode fromJson(Object? value) {
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

class ResourceEstimate {
  const ResourceEstimate({
    required this.acceleration,
    required this.cpuOnly,
    required this.gpuMemoryBytes,
    required this.memory,
    required this.plannedContextTokens,
    required this.storage,
  });

  final AccelerationKind? acceleration;
  final bool cpuOnly;
  final int? gpuMemoryBytes;
  final MemoryEstimate memory;
  final int plannedContextTokens;
  final StorageEstimate storage;

  factory ResourceEstimate.fromJson(Map<String, dynamic> json) {
    return ResourceEstimate(
      acceleration: json['acceleration'] == null ? null : AccelerationKind.fromJson(json['acceleration']),
      cpuOnly: _contractBool(json['cpu_only'], 'ResourceEstimate.cpu_only'),
      gpuMemoryBytes: json['gpu_memory_bytes'] == null ? null : _contractInt(json['gpu_memory_bytes'], 'ResourceEstimate.gpu_memory_bytes'),
      memory: MemoryEstimate.fromJson(_contractMap(json['memory'], 'ResourceEstimate.memory')),
      plannedContextTokens: _contractInt(json['planned_context_tokens'], 'ResourceEstimate.planned_context_tokens'),
      storage: StorageEstimate.fromJson(_contractMap(json['storage'], 'ResourceEstimate.storage')),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'acceleration': acceleration?.toJson(),
      'cpu_only': cpuOnly,
      'gpu_memory_bytes': gpuMemoryBytes,
      'memory': memory.toJson(),
      'planned_context_tokens': plannedContextTokens,
      'storage': storage.toJson(),
    };
  }
}

typedef RuleSetVersion = String;

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

class StorageEstimate {
  const StorageEstimate({
    required this.observedFreeBytes,
    required this.requiredBytes,
    required this.safetyMarginBytes,
  });

  final int? observedFreeBytes;
  final int requiredBytes;
  final int safetyMarginBytes;

  factory StorageEstimate.fromJson(Map<String, dynamic> json) {
    return StorageEstimate(
      observedFreeBytes: json['observed_free_bytes'] == null ? null : _contractInt(json['observed_free_bytes'], 'StorageEstimate.observed_free_bytes'),
      requiredBytes: _contractInt(json['required_bytes'], 'StorageEstimate.required_bytes'),
      safetyMarginBytes: _contractInt(json['safety_margin_bytes'], 'StorageEstimate.safety_margin_bytes'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'observed_free_bytes': observedFreeBytes,
      'required_bytes': requiredBytes,
      'safety_margin_bytes': safetyMarginBytes,
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
  userSelected('user_selected'),
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
  capabilityRecommendation('capability_recommendation'),
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

class UserPreferenceProfile {
  const UserPreferenceProfile({
    required this.includeOptionalLarger,
    required this.priority,
    required this.workload,
  });

  final bool includeOptionalLarger;
  final PreferencePriority priority;
  final WorkloadTier workload;

  factory UserPreferenceProfile.fromJson(Map<String, dynamic> json) {
    return UserPreferenceProfile(
      includeOptionalLarger: _contractBool(json['include_optional_larger'], 'UserPreferenceProfile.include_optional_larger'),
      priority: PreferencePriority.fromJson(json['priority']),
      workload: WorkloadTier.fromJson(json['workload']),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'include_optional_larger': includeOptionalLarger,
      'priority': priority.toJson(),
      'workload': workload.toJson(),
    };
  }
}

enum WorkloadTier {
  generalText('general_text'),
  coding('coding'),
  unknown('unknown'),
  ;

  const WorkloadTier(this.wireValue);

  final String wireValue;

  static WorkloadTier fromJson(Object? value) {
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
