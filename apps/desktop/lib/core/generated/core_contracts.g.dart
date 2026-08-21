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

class CancelGenerationRequest {
  const CancelGenerationRequest({
    required this.correlationId,
    required this.generationId,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final GenerationId generationId;
  final RequestId requestId;

  factory CancelGenerationRequest.fromJson(Map<String, dynamic> json) {
    return CancelGenerationRequest(
      correlationId: _contractString(json['correlation_id'], 'CancelGenerationRequest.correlation_id'),
      generationId: _contractString(json['generation_id'], 'CancelGenerationRequest.generation_id'),
      requestId: _contractString(json['request_id'], 'CancelGenerationRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'generation_id': generationId,
      'request_id': requestId,
    };
  }
}

class CancelGenerationResponse {
  const CancelGenerationResponse({
    required this.accepted,
    required this.correlationId,
    required this.generationId,
    required this.requestId,
  });

  final bool accepted;
  final CorrelationId correlationId;
  final GenerationId generationId;
  final RequestId requestId;

  factory CancelGenerationResponse.fromJson(Map<String, dynamic> json) {
    return CancelGenerationResponse(
      accepted: _contractBool(json['accepted'], 'CancelGenerationResponse.accepted'),
      correlationId: _contractString(json['correlation_id'], 'CancelGenerationResponse.correlation_id'),
      generationId: _contractString(json['generation_id'], 'CancelGenerationResponse.generation_id'),
      requestId: _contractString(json['request_id'], 'CancelGenerationResponse.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'accepted': accepted,
      'correlation_id': correlationId,
      'generation_id': generationId,
      'request_id': requestId,
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

enum ChatFailureCode {
  runtimeUnavailable('runtime_unavailable'),
  runtimeIncompatible('runtime_incompatible'),
  runtimeConsentRequired('runtime_consent_required'),
  modelUnavailable('model_unavailable'),
  modelChanged('model_changed'),
  generationTimedOut('generation_timed_out'),
  providerDisconnected('provider_disconnected'),
  malformedProviderStream('malformed_provider_stream'),
  messageTooLarge('message_too_large'),
  contextTooLarge('context_too_large'),
  conversationNotFound('conversation_not_found'),
  generationNotFound('generation_not_found'),
  generationAlreadyActive('generation_already_active'),
  persistenceUnavailable('persistence_unavailable'),
  cancelled('cancelled'),
  unknown('unknown'),
  ;

  const ChatFailureCode(this.wireValue);

  final String wireValue;

  static ChatFailureCode fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class ChatGenerationEvent {
  const ChatGenerationEvent({
    required this.assistantMessageId,
    required this.conversationId,
    required this.correlationId,
    required this.delta,
    required this.error,
    required this.generationId,
    required this.kind,
    required this.occurredAtUnixMs,
    required this.schemaVersion,
    required this.sequence,
    required this.terminalState,
  });

  final MessageId assistantMessageId;
  final ConversationId conversationId;
  final CorrelationId correlationId;
  final String? delta;
  final SafeErrorPayload? error;
  final GenerationId generationId;
  final ChatGenerationEventKind kind;
  final int occurredAtUnixMs;
  final int schemaVersion;
  final int sequence;
  final ChatGenerationTerminalState? terminalState;

  factory ChatGenerationEvent.fromJson(Map<String, dynamic> json) {
    return ChatGenerationEvent(
      assistantMessageId: _contractString(json['assistant_message_id'], 'ChatGenerationEvent.assistant_message_id'),
      conversationId: _contractString(json['conversation_id'], 'ChatGenerationEvent.conversation_id'),
      correlationId: _contractString(json['correlation_id'], 'ChatGenerationEvent.correlation_id'),
      delta: json['delta'] == null ? null : _contractString(json['delta'], 'ChatGenerationEvent.delta'),
      error: json['error'] == null ? null : SafeErrorPayload.fromJson(_contractMap(json['error'], 'ChatGenerationEvent.error')),
      generationId: _contractString(json['generation_id'], 'ChatGenerationEvent.generation_id'),
      kind: ChatGenerationEventKind.fromJson(json['kind']),
      occurredAtUnixMs: _contractInt(json['occurred_at_unix_ms'], 'ChatGenerationEvent.occurred_at_unix_ms'),
      schemaVersion: _contractInt(json['schema_version'], 'ChatGenerationEvent.schema_version'),
      sequence: _contractInt(json['sequence'], 'ChatGenerationEvent.sequence'),
      terminalState: json['terminal_state'] == null ? null : ChatGenerationTerminalState.fromJson(json['terminal_state']),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'assistant_message_id': assistantMessageId,
      'conversation_id': conversationId,
      'correlation_id': correlationId,
      'delta': delta,
      'error': error?.toJson(),
      'generation_id': generationId,
      'kind': kind.toJson(),
      'occurred_at_unix_ms': occurredAtUnixMs,
      'schema_version': schemaVersion,
      'sequence': sequence,
      'terminal_state': terminalState?.toJson(),
    };
  }
}

enum ChatGenerationEventKind {
  started('started'),
  delta('delta'),
  completed('completed'),
  cancelled('cancelled'),
  failed('failed'),
  timedOut('timed_out'),
  durabilityInterrupted('durability_interrupted'),
  unknown('unknown'),
  ;

  const ChatGenerationEventKind(this.wireValue);

  final String wireValue;

  static ChatGenerationEventKind fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class ChatGenerationEventsRequest {
  const ChatGenerationEventsRequest({
    required this.afterSequence,
    required this.correlationId,
    required this.generationId,
    required this.limit,
    required this.requestId,
  });

  final int afterSequence;
  final CorrelationId correlationId;
  final GenerationId generationId;
  final int limit;
  final RequestId requestId;

  factory ChatGenerationEventsRequest.fromJson(Map<String, dynamic> json) {
    return ChatGenerationEventsRequest(
      afterSequence: _contractInt(json['after_sequence'], 'ChatGenerationEventsRequest.after_sequence'),
      correlationId: _contractString(json['correlation_id'], 'ChatGenerationEventsRequest.correlation_id'),
      generationId: _contractString(json['generation_id'], 'ChatGenerationEventsRequest.generation_id'),
      limit: _contractInt(json['limit'], 'ChatGenerationEventsRequest.limit'),
      requestId: _contractString(json['request_id'], 'ChatGenerationEventsRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'after_sequence': afterSequence,
      'correlation_id': correlationId,
      'generation_id': generationId,
      'limit': limit,
      'request_id': requestId,
    };
  }
}

class ChatGenerationEventsResponse {
  const ChatGenerationEventsResponse({
    required this.correlationId,
    required this.events,
    required this.replayIncomplete,
    required this.requestId,
    required this.schemaVersion,
  });

  final CorrelationId correlationId;
  final List<ChatGenerationEvent> events;
  final bool replayIncomplete;
  final RequestId requestId;
  final int schemaVersion;

  factory ChatGenerationEventsResponse.fromJson(Map<String, dynamic> json) {
    return ChatGenerationEventsResponse(
      correlationId: _contractString(json['correlation_id'], 'ChatGenerationEventsResponse.correlation_id'),
      events: _contractList(json['events'], 'ChatGenerationEventsResponse.events').map((item) => ChatGenerationEvent.fromJson(_contractMap(item, 'ChatGenerationEventsResponse.events[]'))).toList(growable: false),
      replayIncomplete: _contractBool(json['replay_incomplete'], 'ChatGenerationEventsResponse.replay_incomplete'),
      requestId: _contractString(json['request_id'], 'ChatGenerationEventsResponse.request_id'),
      schemaVersion: _contractInt(json['schema_version'], 'ChatGenerationEventsResponse.schema_version'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'events': events.map((item) => item.toJson()).toList(growable: false),
      'replay_incomplete': replayIncomplete,
      'request_id': requestId,
      'schema_version': schemaVersion,
    };
  }
}

enum ChatGenerationTerminalState {
  completed('completed'),
  cancelled('cancelled'),
  failed('failed'),
  timedOut('timed_out'),
  unknown('unknown'),
  ;

  const ChatGenerationTerminalState(this.wireValue);

  final String wireValue;

  static ChatGenerationTerminalState fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

enum ChatLocalityStatus {
  runningLocally('running_locally'),
  unknown('unknown'),
  ;

  const ChatLocalityStatus(this.wireValue);

  final String wireValue;

  static ChatLocalityStatus fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class ChatMessage {
  const ChatMessage({
    required this.completedAtUnixMs,
    required this.content,
    required this.conversationId,
    required this.createdAtUnixMs,
    required this.generationId,
    required this.lastEventSequence,
    required this.messageId,
    required this.role,
    required this.sequence,
    required this.status,
    required this.updatedAtUnixMs,
  });

  final int? completedAtUnixMs;
  final String content;
  final ConversationId conversationId;
  final int createdAtUnixMs;
  final GenerationId? generationId;
  final int lastEventSequence;
  final MessageId messageId;
  final ChatRole role;
  final int sequence;
  final ChatMessageStatus status;
  final int updatedAtUnixMs;

  factory ChatMessage.fromJson(Map<String, dynamic> json) {
    return ChatMessage(
      completedAtUnixMs: json['completed_at_unix_ms'] == null ? null : _contractInt(json['completed_at_unix_ms'], 'ChatMessage.completed_at_unix_ms'),
      content: _contractString(json['content'], 'ChatMessage.content'),
      conversationId: _contractString(json['conversation_id'], 'ChatMessage.conversation_id'),
      createdAtUnixMs: _contractInt(json['created_at_unix_ms'], 'ChatMessage.created_at_unix_ms'),
      generationId: json['generation_id'] == null ? null : _contractString(json['generation_id'], 'ChatMessage.generation_id'),
      lastEventSequence: _contractInt(json['last_event_sequence'], 'ChatMessage.last_event_sequence'),
      messageId: _contractString(json['message_id'], 'ChatMessage.message_id'),
      role: ChatRole.fromJson(json['role']),
      sequence: _contractInt(json['sequence'], 'ChatMessage.sequence'),
      status: ChatMessageStatus.fromJson(json['status']),
      updatedAtUnixMs: _contractInt(json['updated_at_unix_ms'], 'ChatMessage.updated_at_unix_ms'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'completed_at_unix_ms': completedAtUnixMs,
      'content': content,
      'conversation_id': conversationId,
      'created_at_unix_ms': createdAtUnixMs,
      'generation_id': generationId,
      'last_event_sequence': lastEventSequence,
      'message_id': messageId,
      'role': role.toJson(),
      'sequence': sequence,
      'status': status.toJson(),
      'updated_at_unix_ms': updatedAtUnixMs,
    };
  }
}

enum ChatMessageStatus {
  pending('pending'),
  generating('generating'),
  completed('completed'),
  cancelled('cancelled'),
  failed('failed'),
  unknown('unknown'),
  ;

  const ChatMessageStatus(this.wireValue);

  final String wireValue;

  static ChatMessageStatus fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class ChatModelIdentity {
  const ChatModelIdentity({
    required this.canonicalModelId,
    required this.displayName,
    required this.family,
  });

  final CandidateModelId canonicalModelId;
  final String displayName;
  final String family;

  factory ChatModelIdentity.fromJson(Map<String, dynamic> json) {
    return ChatModelIdentity(
      canonicalModelId: _contractString(json['canonical_model_id'], 'ChatModelIdentity.canonical_model_id'),
      displayName: _contractString(json['display_name'], 'ChatModelIdentity.display_name'),
      family: _contractString(json['family'], 'ChatModelIdentity.family'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'canonical_model_id': canonicalModelId,
      'display_name': displayName,
      'family': family,
    };
  }
}

enum ChatRecoveryAction {
  retryGeneration('retry_generation'),
  checkRuntime('check_runtime'),
  runModelSetup('run_model_setup'),
  shortenMessage('shorten_message'),
  startNewConversation('start_new_conversation'),
  restartApplication('restart_application'),
  noAction('no_action'),
  unknown('unknown'),
  ;

  const ChatRecoveryAction(this.wireValue);

  final String wireValue;

  static ChatRecoveryAction fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

enum ChatRole {
  user('user'),
  assistant('assistant'),
  unknown('unknown'),
  ;

  const ChatRole(this.wireValue);

  final String wireValue;

  static ChatRole fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class ChatRuntimeStatus {
  const ChatRuntimeStatus({
    required this.blockedBy,
    required this.locality,
    required this.model,
    required this.providerId,
    required this.ready,
    required this.recoveryAction,
    required this.runtimeDisplayName,
    required this.schemaVersion,
  });

  final ChatFailureCode? blockedBy;
  final ChatLocalityStatus locality;
  final ChatModelIdentity? model;
  final RuntimeProviderId providerId;
  final bool ready;
  final ChatRecoveryAction? recoveryAction;
  final RuntimeDisplayName runtimeDisplayName;
  final int schemaVersion;

  factory ChatRuntimeStatus.fromJson(Map<String, dynamic> json) {
    return ChatRuntimeStatus(
      blockedBy: json['blocked_by'] == null ? null : ChatFailureCode.fromJson(json['blocked_by']),
      locality: ChatLocalityStatus.fromJson(json['locality']),
      model: json['model'] == null ? null : ChatModelIdentity.fromJson(_contractMap(json['model'], 'ChatRuntimeStatus.model')),
      providerId: _contractString(json['provider_id'], 'ChatRuntimeStatus.provider_id'),
      ready: _contractBool(json['ready'], 'ChatRuntimeStatus.ready'),
      recoveryAction: json['recovery_action'] == null ? null : ChatRecoveryAction.fromJson(json['recovery_action']),
      runtimeDisplayName: _contractString(json['runtime_display_name'], 'ChatRuntimeStatus.runtime_display_name'),
      schemaVersion: _contractInt(json['schema_version'], 'ChatRuntimeStatus.schema_version'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'blocked_by': blockedBy?.toJson(),
      'locality': locality.toJson(),
      'model': model?.toJson(),
      'provider_id': providerId,
      'ready': ready,
      'recovery_action': recoveryAction?.toJson(),
      'runtime_display_name': runtimeDisplayName,
      'schema_version': schemaVersion,
    };
  }
}

class ChatRuntimeStatusRequest {
  const ChatRuntimeStatusRequest({
    required this.conversationId,
    required this.correlationId,
    required this.requestId,
  });

  final ConversationId? conversationId;
  final CorrelationId correlationId;
  final RequestId requestId;

  factory ChatRuntimeStatusRequest.fromJson(Map<String, dynamic> json) {
    return ChatRuntimeStatusRequest(
      conversationId: json['conversation_id'] == null ? null : _contractString(json['conversation_id'], 'ChatRuntimeStatusRequest.conversation_id'),
      correlationId: _contractString(json['correlation_id'], 'ChatRuntimeStatusRequest.correlation_id'),
      requestId: _contractString(json['request_id'], 'ChatRuntimeStatusRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'conversation_id': conversationId,
      'correlation_id': correlationId,
      'request_id': requestId,
    };
  }
}

class ChatRuntimeStatusResponse {
  const ChatRuntimeStatusResponse({
    required this.correlationId,
    required this.requestId,
    required this.status,
  });

  final CorrelationId correlationId;
  final RequestId requestId;
  final ChatRuntimeStatus status;

  factory ChatRuntimeStatusResponse.fromJson(Map<String, dynamic> json) {
    return ChatRuntimeStatusResponse(
      correlationId: _contractString(json['correlation_id'], 'ChatRuntimeStatusResponse.correlation_id'),
      requestId: _contractString(json['request_id'], 'ChatRuntimeStatusResponse.request_id'),
      status: ChatRuntimeStatus.fromJson(_contractMap(json['status'], 'ChatRuntimeStatusResponse.status')),
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

class ChatWarning {
  const ChatWarning({
    required this.code,
    required this.message,
  });

  final ChatWarningCode code;
  final String message;

  factory ChatWarning.fromJson(Map<String, dynamic> json) {
    return ChatWarning(
      code: ChatWarningCode.fromJson(json['code']),
      message: _contractString(json['message'], 'ChatWarning.message'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'code': code.toJson(),
      'message': message,
    };
  }
}

enum ChatWarningCode {
  contextTruncated('context_truncated'),
  partialAssistantContent('partial_assistant_content'),
  interruptedGenerationRecovered('interrupted_generation_recovered'),
  unknown('unknown'),
  ;

  const ChatWarningCode(this.wireValue);

  final String wireValue;

  static ChatWarningCode fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
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

typedef ConversationId = String;

class ConversationSnapshot {
  const ConversationSnapshot({
    required this.activeGenerationId,
    required this.conversationId,
    required this.createdAtUnixMs,
    required this.messages,
    required this.model,
    required this.schemaVersion,
    required this.title,
    required this.updatedAtUnixMs,
    required this.warnings,
  });

  final GenerationId? activeGenerationId;
  final ConversationId conversationId;
  final int createdAtUnixMs;
  final List<ChatMessage> messages;
  final ChatModelIdentity? model;
  final int schemaVersion;
  final String title;
  final int updatedAtUnixMs;
  final List<ChatWarning> warnings;

  factory ConversationSnapshot.fromJson(Map<String, dynamic> json) {
    return ConversationSnapshot(
      activeGenerationId: json['active_generation_id'] == null ? null : _contractString(json['active_generation_id'], 'ConversationSnapshot.active_generation_id'),
      conversationId: _contractString(json['conversation_id'], 'ConversationSnapshot.conversation_id'),
      createdAtUnixMs: _contractInt(json['created_at_unix_ms'], 'ConversationSnapshot.created_at_unix_ms'),
      messages: _contractList(json['messages'], 'ConversationSnapshot.messages').map((item) => ChatMessage.fromJson(_contractMap(item, 'ConversationSnapshot.messages[]'))).toList(growable: false),
      model: json['model'] == null ? null : ChatModelIdentity.fromJson(_contractMap(json['model'], 'ConversationSnapshot.model')),
      schemaVersion: _contractInt(json['schema_version'], 'ConversationSnapshot.schema_version'),
      title: _contractString(json['title'], 'ConversationSnapshot.title'),
      updatedAtUnixMs: _contractInt(json['updated_at_unix_ms'], 'ConversationSnapshot.updated_at_unix_ms'),
      warnings: _contractList(json['warnings'], 'ConversationSnapshot.warnings').map((item) => ChatWarning.fromJson(_contractMap(item, 'ConversationSnapshot.warnings[]'))).toList(growable: false),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'active_generation_id': activeGenerationId,
      'conversation_id': conversationId,
      'created_at_unix_ms': createdAtUnixMs,
      'messages': messages.map((item) => item.toJson()).toList(growable: false),
      'model': model?.toJson(),
      'schema_version': schemaVersion,
      'title': title,
      'updated_at_unix_ms': updatedAtUnixMs,
      'warnings': warnings.map((item) => item.toJson()).toList(growable: false),
    };
  }
}

class ConversationSummary {
  const ConversationSummary({
    required this.conversationId,
    required this.createdAtUnixMs,
    required this.messageCount,
    required this.model,
    required this.title,
    required this.updatedAtUnixMs,
  });

  final ConversationId conversationId;
  final int createdAtUnixMs;
  final int messageCount;
  final ChatModelIdentity? model;
  final String title;
  final int updatedAtUnixMs;

  factory ConversationSummary.fromJson(Map<String, dynamic> json) {
    return ConversationSummary(
      conversationId: _contractString(json['conversation_id'], 'ConversationSummary.conversation_id'),
      createdAtUnixMs: _contractInt(json['created_at_unix_ms'], 'ConversationSummary.created_at_unix_ms'),
      messageCount: _contractInt(json['message_count'], 'ConversationSummary.message_count'),
      model: json['model'] == null ? null : ChatModelIdentity.fromJson(_contractMap(json['model'], 'ConversationSummary.model')),
      title: _contractString(json['title'], 'ConversationSummary.title'),
      updatedAtUnixMs: _contractInt(json['updated_at_unix_ms'], 'ConversationSummary.updated_at_unix_ms'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'conversation_id': conversationId,
      'created_at_unix_ms': createdAtUnixMs,
      'message_count': messageCount,
      'model': model?.toJson(),
      'title': title,
      'updated_at_unix_ms': updatedAtUnixMs,
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
    required this.runtimeProviderId,
    required this.selectedProtocol,
    required this.supportedCapabilities,
  });

  final ApplicationInfo application;
  final CorrelationId correlationId;
  final InstanceId instanceId;
  final ReadinessReport readiness;
  final RequestId requestId;
  final RuntimeProviderId? runtimeProviderId;
  final int selectedProtocol;
  final List<TransportCapability> supportedCapabilities;

  factory CoreHello.fromJson(Map<String, dynamic> json) {
    return CoreHello(
      application: ApplicationInfo.fromJson(_contractMap(json['application'], 'CoreHello.application')),
      correlationId: _contractString(json['correlation_id'], 'CoreHello.correlation_id'),
      instanceId: _contractString(json['instance_id'], 'CoreHello.instance_id'),
      readiness: ReadinessReport.fromJson(_contractMap(json['readiness'], 'CoreHello.readiness')),
      requestId: _contractString(json['request_id'], 'CoreHello.request_id'),
      runtimeProviderId: json['runtime_provider_id'] == null ? null : _contractString(json['runtime_provider_id'], 'CoreHello.runtime_provider_id'),
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
      'runtime_provider_id': runtimeProviderId,
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

class CreateConversationRequest {
  const CreateConversationRequest({
    required this.correlationId,
    required this.requestId,
    required this.title,
  });

  final CorrelationId correlationId;
  final RequestId requestId;
  final String? title;

  factory CreateConversationRequest.fromJson(Map<String, dynamic> json) {
    return CreateConversationRequest(
      correlationId: _contractString(json['correlation_id'], 'CreateConversationRequest.correlation_id'),
      requestId: _contractString(json['request_id'], 'CreateConversationRequest.request_id'),
      title: json['title'] == null ? null : _contractString(json['title'], 'CreateConversationRequest.title'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'request_id': requestId,
      'title': title,
    };
  }
}

class CreateConversationResponse {
  const CreateConversationResponse({
    required this.conversation,
    required this.correlationId,
    required this.requestId,
  });

  final ConversationSnapshot conversation;
  final CorrelationId correlationId;
  final RequestId requestId;

  factory CreateConversationResponse.fromJson(Map<String, dynamic> json) {
    return CreateConversationResponse(
      conversation: ConversationSnapshot.fromJson(_contractMap(json['conversation'], 'CreateConversationResponse.conversation')),
      correlationId: _contractString(json['correlation_id'], 'CreateConversationResponse.correlation_id'),
      requestId: _contractString(json['request_id'], 'CreateConversationResponse.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'conversation': conversation.toJson(),
      'correlation_id': correlationId,
      'request_id': requestId,
    };
  }
}

class DeleteConversationRequest {
  const DeleteConversationRequest({
    required this.conversationId,
    required this.correlationId,
    required this.requestId,
  });

  final ConversationId conversationId;
  final CorrelationId correlationId;
  final RequestId requestId;

  factory DeleteConversationRequest.fromJson(Map<String, dynamic> json) {
    return DeleteConversationRequest(
      conversationId: _contractString(json['conversation_id'], 'DeleteConversationRequest.conversation_id'),
      correlationId: _contractString(json['correlation_id'], 'DeleteConversationRequest.correlation_id'),
      requestId: _contractString(json['request_id'], 'DeleteConversationRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'conversation_id': conversationId,
      'correlation_id': correlationId,
      'request_id': requestId,
    };
  }
}

class DeleteConversationResponse {
  const DeleteConversationResponse({
    required this.conversationId,
    required this.correlationId,
    required this.deleted,
    required this.deletedMessageCount,
    required this.requestId,
  });

  final ConversationId conversationId;
  final CorrelationId correlationId;
  final bool deleted;
  final int deletedMessageCount;
  final RequestId requestId;

  factory DeleteConversationResponse.fromJson(Map<String, dynamic> json) {
    return DeleteConversationResponse(
      conversationId: _contractString(json['conversation_id'], 'DeleteConversationResponse.conversation_id'),
      correlationId: _contractString(json['correlation_id'], 'DeleteConversationResponse.correlation_id'),
      deleted: _contractBool(json['deleted'], 'DeleteConversationResponse.deleted'),
      deletedMessageCount: _contractInt(json['deleted_message_count'], 'DeleteConversationResponse.deleted_message_count'),
      requestId: _contractString(json['request_id'], 'DeleteConversationResponse.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'conversation_id': conversationId,
      'correlation_id': correlationId,
      'deleted': deleted,
      'deleted_message_count': deletedMessageCount,
      'request_id': requestId,
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

typedef GenerationId = String;

class GetConversationRequest {
  const GetConversationRequest({
    required this.conversationId,
    required this.correlationId,
    required this.requestId,
  });

  final ConversationId conversationId;
  final CorrelationId correlationId;
  final RequestId requestId;

  factory GetConversationRequest.fromJson(Map<String, dynamic> json) {
    return GetConversationRequest(
      conversationId: _contractString(json['conversation_id'], 'GetConversationRequest.conversation_id'),
      correlationId: _contractString(json['correlation_id'], 'GetConversationRequest.correlation_id'),
      requestId: _contractString(json['request_id'], 'GetConversationRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'conversation_id': conversationId,
      'correlation_id': correlationId,
      'request_id': requestId,
    };
  }
}

class GetConversationResponse {
  const GetConversationResponse({
    required this.conversation,
    required this.correlationId,
    required this.requestId,
  });

  final ConversationSnapshot conversation;
  final CorrelationId correlationId;
  final RequestId requestId;

  factory GetConversationResponse.fromJson(Map<String, dynamic> json) {
    return GetConversationResponse(
      conversation: ConversationSnapshot.fromJson(_contractMap(json['conversation'], 'GetConversationResponse.conversation')),
      correlationId: _contractString(json['correlation_id'], 'GetConversationResponse.correlation_id'),
      requestId: _contractString(json['request_id'], 'GetConversationResponse.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'conversation': conversation.toJson(),
      'correlation_id': correlationId,
      'request_id': requestId,
    };
  }
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

class ListConversationsRequest {
  const ListConversationsRequest({
    required this.correlationId,
    required this.limit,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final int limit;
  final RequestId requestId;

  factory ListConversationsRequest.fromJson(Map<String, dynamic> json) {
    return ListConversationsRequest(
      correlationId: _contractString(json['correlation_id'], 'ListConversationsRequest.correlation_id'),
      limit: _contractInt(json['limit'], 'ListConversationsRequest.limit'),
      requestId: _contractString(json['request_id'], 'ListConversationsRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'limit': limit,
      'request_id': requestId,
    };
  }
}

class ListConversationsResponse {
  const ListConversationsResponse({
    required this.conversations,
    required this.correlationId,
    required this.requestId,
    required this.schemaVersion,
    required this.truncated,
  });

  final List<ConversationSummary> conversations;
  final CorrelationId correlationId;
  final RequestId requestId;
  final int schemaVersion;
  final bool truncated;

  factory ListConversationsResponse.fromJson(Map<String, dynamic> json) {
    return ListConversationsResponse(
      conversations: _contractList(json['conversations'], 'ListConversationsResponse.conversations').map((item) => ConversationSummary.fromJson(_contractMap(item, 'ListConversationsResponse.conversations[]'))).toList(growable: false),
      correlationId: _contractString(json['correlation_id'], 'ListConversationsResponse.correlation_id'),
      requestId: _contractString(json['request_id'], 'ListConversationsResponse.request_id'),
      schemaVersion: _contractInt(json['schema_version'], 'ListConversationsResponse.schema_version'),
      truncated: _contractBool(json['truncated'], 'ListConversationsResponse.truncated'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'conversations': conversations.map((item) => item.toJson()).toList(growable: false),
      'correlation_id': correlationId,
      'request_id': requestId,
      'schema_version': schemaVersion,
      'truncated': truncated,
    };
  }
}

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

typedef MessageId = String;

enum ModelAcquisitionPhase {
  preparing('preparing'),
  transferring('transferring'),
  verifying('verifying'),
  registering('registering'),
  completed('completed'),
  unknown('unknown'),
  ;

  const ModelAcquisitionPhase(this.wireValue);

  final String wireValue;

  static ModelAcquisitionPhase fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class ModelAcquisitionProgress {
  const ModelAcquisitionProgress({
    required this.completedBytes,
    required this.phase,
    required this.progressBasisPoints,
    required this.totalBytes,
  });

  final int? completedBytes;
  final ModelAcquisitionPhase phase;
  final int? progressBasisPoints;
  final int? totalBytes;

  factory ModelAcquisitionProgress.fromJson(Map<String, dynamic> json) {
    return ModelAcquisitionProgress(
      completedBytes: json['completed_bytes'] == null ? null : _contractInt(json['completed_bytes'], 'ModelAcquisitionProgress.completed_bytes'),
      phase: ModelAcquisitionPhase.fromJson(json['phase']),
      progressBasisPoints: json['progress_basis_points'] == null ? null : _contractInt(json['progress_basis_points'], 'ModelAcquisitionProgress.progress_basis_points'),
      totalBytes: json['total_bytes'] == null ? null : _contractInt(json['total_bytes'], 'ModelAcquisitionProgress.total_bytes'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'completed_bytes': completedBytes,
      'phase': phase.toJson(),
      'progress_basis_points': progressBasisPoints,
      'total_bytes': totalBytes,
    };
  }
}

enum ModelIntegrityState {
  unavailable('unavailable'),
  providerReported('provider_reported'),
  verified('verified'),
  mismatch('mismatch'),
  unknown('unknown'),
  ;

  const ModelIntegrityState(this.wireValue);

  final String wireValue;

  static ModelIntegrityState fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

enum ModelLifecycleState {
  planned('planned'),
  acquiring('acquiring'),
  acquired('acquired'),
  registering('registering'),
  verifying('verifying'),
  available('available'),
  attentionRequired('attention_required'),
  failed('failed'),
  cancelled('cancelled'),
  unknown('unknown'),
  ;

  const ModelLifecycleState(this.wireValue);

  final String wireValue;

  static ModelLifecycleState fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class ModelMetadata {
  const ModelMetadata({
    required this.artifact,
    required this.catalogueVersion,
    required this.destination,
    required this.destinationDisplay,
    required this.displayName,
    required this.expectedSizeBytes,
    required this.family,
    required this.licenceSpdx,
    required this.lifecycleState,
    required this.measuredSizeBytes,
    required this.provenance,
    required this.ruleSetVersion,
    required this.schemaVersion,
    required this.sizeClass,
    required this.verification,
    required this.verificationState,
  });

  final ModelProviderArtifact artifact;
  final CatalogueVersion catalogueVersion;
  final SetupDestinationCategory destination;
  final String destinationDisplay;
  final String displayName;
  final int expectedSizeBytes;
  final String family;
  final String licenceSpdx;
  final ModelLifecycleState lifecycleState;
  final int? measuredSizeBytes;
  final String provenance;
  final RuleSetVersion ruleSetVersion;
  final int schemaVersion;
  final ModelSizeClass sizeClass;
  final ModelVerificationResult? verification;
  final ModelVerificationState verificationState;

  factory ModelMetadata.fromJson(Map<String, dynamic> json) {
    return ModelMetadata(
      artifact: ModelProviderArtifact.fromJson(_contractMap(json['artifact'], 'ModelMetadata.artifact')),
      catalogueVersion: _contractString(json['catalogue_version'], 'ModelMetadata.catalogue_version'),
      destination: SetupDestinationCategory.fromJson(json['destination']),
      destinationDisplay: _contractString(json['destination_display'], 'ModelMetadata.destination_display'),
      displayName: _contractString(json['display_name'], 'ModelMetadata.display_name'),
      expectedSizeBytes: _contractInt(json['expected_size_bytes'], 'ModelMetadata.expected_size_bytes'),
      family: _contractString(json['family'], 'ModelMetadata.family'),
      licenceSpdx: _contractString(json['licence_spdx'], 'ModelMetadata.licence_spdx'),
      lifecycleState: ModelLifecycleState.fromJson(json['lifecycle_state']),
      measuredSizeBytes: json['measured_size_bytes'] == null ? null : _contractInt(json['measured_size_bytes'], 'ModelMetadata.measured_size_bytes'),
      provenance: _contractString(json['provenance'], 'ModelMetadata.provenance'),
      ruleSetVersion: _contractString(json['rule_set_version'], 'ModelMetadata.rule_set_version'),
      schemaVersion: _contractInt(json['schema_version'], 'ModelMetadata.schema_version'),
      sizeClass: ModelSizeClass.fromJson(json['size_class']),
      verification: json['verification'] == null ? null : ModelVerificationResult.fromJson(_contractMap(json['verification'], 'ModelMetadata.verification')),
      verificationState: ModelVerificationState.fromJson(json['verification_state']),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'artifact': artifact.toJson(),
      'catalogue_version': catalogueVersion,
      'destination': destination.toJson(),
      'destination_display': destinationDisplay,
      'display_name': displayName,
      'expected_size_bytes': expectedSizeBytes,
      'family': family,
      'licence_spdx': licenceSpdx,
      'lifecycle_state': lifecycleState.toJson(),
      'measured_size_bytes': measuredSizeBytes,
      'provenance': provenance,
      'rule_set_version': ruleSetVersion,
      'schema_version': schemaVersion,
      'size_class': sizeClass.toJson(),
      'verification': verification?.toJson(),
      'verification_state': verificationState.toJson(),
    };
  }
}

class ModelProviderArtifact {
  const ModelProviderArtifact({
    required this.canonicalModelId,
    required this.providerId,
    required this.providerModelId,
    required this.sourceSummary,
  });

  final CandidateModelId canonicalModelId;
  final RuntimeProviderId providerId;
  final RuntimeProviderModelId providerModelId;
  final String sourceSummary;

  factory ModelProviderArtifact.fromJson(Map<String, dynamic> json) {
    return ModelProviderArtifact(
      canonicalModelId: _contractString(json['canonical_model_id'], 'ModelProviderArtifact.canonical_model_id'),
      providerId: _contractString(json['provider_id'], 'ModelProviderArtifact.provider_id'),
      providerModelId: _contractString(json['provider_model_id'], 'ModelProviderArtifact.provider_model_id'),
      sourceSummary: _contractString(json['source_summary'], 'ModelProviderArtifact.source_summary'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'canonical_model_id': canonicalModelId,
      'provider_id': providerId,
      'provider_model_id': providerModelId,
      'source_summary': sourceSummary,
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

class ModelVerificationResult {
  const ModelVerificationResult({
    required this.inferenceVerified,
    required this.integrity,
    required this.modelAvailable,
    required this.registrationVerified,
    required this.runtimeHealthVerified,
    required this.verifiedAtUnixMs,
  });

  final bool inferenceVerified;
  final ModelIntegrityState integrity;
  final bool modelAvailable;
  final bool registrationVerified;
  final bool runtimeHealthVerified;
  final int verifiedAtUnixMs;

  factory ModelVerificationResult.fromJson(Map<String, dynamic> json) {
    return ModelVerificationResult(
      inferenceVerified: _contractBool(json['inference_verified'], 'ModelVerificationResult.inference_verified'),
      integrity: ModelIntegrityState.fromJson(json['integrity']),
      modelAvailable: _contractBool(json['model_available'], 'ModelVerificationResult.model_available'),
      registrationVerified: _contractBool(json['registration_verified'], 'ModelVerificationResult.registration_verified'),
      runtimeHealthVerified: _contractBool(json['runtime_health_verified'], 'ModelVerificationResult.runtime_health_verified'),
      verifiedAtUnixMs: _contractInt(json['verified_at_unix_ms'], 'ModelVerificationResult.verified_at_unix_ms'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'inference_verified': inferenceVerified,
      'integrity': integrity.toJson(),
      'model_available': modelAvailable,
      'registration_verified': registrationVerified,
      'runtime_health_verified': runtimeHealthVerified,
      'verified_at_unix_ms': verifiedAtUnixMs,
    };
  }
}

enum ModelVerificationState {
  notStarted('not_started'),
  runtimeVerified('runtime_verified'),
  availabilityVerified('availability_verified'),
  registrationVerified('registration_verified'),
  verified('verified'),
  failed('failed'),
  cancelled('cancelled'),
  unknown('unknown'),
  ;

  const ModelVerificationState(this.wireValue);

  final String wireValue;

  static ModelVerificationState fromJson(Object? value) {
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

class RenameConversationRequest {
  const RenameConversationRequest({
    required this.conversationId,
    required this.correlationId,
    required this.requestId,
    required this.title,
  });

  final ConversationId conversationId;
  final CorrelationId correlationId;
  final RequestId requestId;
  final String title;

  factory RenameConversationRequest.fromJson(Map<String, dynamic> json) {
    return RenameConversationRequest(
      conversationId: _contractString(json['conversation_id'], 'RenameConversationRequest.conversation_id'),
      correlationId: _contractString(json['correlation_id'], 'RenameConversationRequest.correlation_id'),
      requestId: _contractString(json['request_id'], 'RenameConversationRequest.request_id'),
      title: _contractString(json['title'], 'RenameConversationRequest.title'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'conversation_id': conversationId,
      'correlation_id': correlationId,
      'request_id': requestId,
      'title': title,
    };
  }
}

class RenameConversationResponse {
  const RenameConversationResponse({
    required this.conversation,
    required this.correlationId,
    required this.requestId,
  });

  final ConversationSnapshot conversation;
  final CorrelationId correlationId;
  final RequestId requestId;

  factory RenameConversationResponse.fromJson(Map<String, dynamic> json) {
    return RenameConversationResponse(
      conversation: ConversationSnapshot.fromJson(_contractMap(json['conversation'], 'RenameConversationResponse.conversation')),
      correlationId: _contractString(json['correlation_id'], 'RenameConversationResponse.correlation_id'),
      requestId: _contractString(json['request_id'], 'RenameConversationResponse.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'conversation': conversation.toJson(),
      'correlation_id': correlationId,
      'request_id': requestId,
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

enum RuntimeCapabilityAvailability {
  available('available'),
  requiresReuseConsent('requires_reuse_consent'),
  requiresManagementConsent('requires_management_consent'),
  requiresSetupApproval('requires_setup_approval'),
  unsupported('unsupported'),
  unknown('unknown'),
  ;

  const RuntimeCapabilityAvailability(this.wireValue);

  final String wireValue;

  static RuntimeCapabilityAvailability fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class RuntimeCapabilityDescriptor {
  const RuntimeCapabilityDescriptor({
    required this.availability,
    required this.kind,
    required this.reason,
  });

  final RuntimeCapabilityAvailability availability;
  final RuntimeCapabilityKind kind;
  final String? reason;

  factory RuntimeCapabilityDescriptor.fromJson(Map<String, dynamic> json) {
    return RuntimeCapabilityDescriptor(
      availability: RuntimeCapabilityAvailability.fromJson(json['availability']),
      kind: RuntimeCapabilityKind.fromJson(json['kind']),
      reason: json['reason'] == null ? null : _contractString(json['reason'], 'RuntimeCapabilityDescriptor.reason'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'availability': availability.toJson(),
      'kind': kind.toJson(),
      'reason': reason,
    };
  }
}

enum RuntimeCapabilityKind {
  detection('detection'),
  health('health'),
  version('version'),
  start('start'),
  stop('stop'),
  restart('restart'),
  modelInventory('model_inventory'),
  modelAcquisitionPreparation('model_acquisition_preparation'),
  modelStoragePreflight('model_storage_preflight'),
  modelAcquisition('model_acquisition'),
  modelRegistration('model_registration'),
  readinessInference('readiness_inference'),
  unknown('unknown'),
  ;

  const RuntimeCapabilityKind(this.wireValue);

  final String wireValue;

  static RuntimeCapabilityKind fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

enum RuntimeConsentDecision {
  approveReuse('approve_reuse'),
  denyReuse('deny_reuse'),
  unknown('unknown'),
  ;

  const RuntimeConsentDecision(this.wireValue);

  final String wireValue;

  static RuntimeConsentDecision fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class RuntimeConsentRequest {
  const RuntimeConsentRequest({
    required this.correlationId,
    required this.decision,
    required this.providerId,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final RuntimeConsentDecision decision;
  final RuntimeProviderId providerId;
  final RequestId requestId;

  factory RuntimeConsentRequest.fromJson(Map<String, dynamic> json) {
    return RuntimeConsentRequest(
      correlationId: _contractString(json['correlation_id'], 'RuntimeConsentRequest.correlation_id'),
      decision: RuntimeConsentDecision.fromJson(json['decision']),
      providerId: _contractString(json['provider_id'], 'RuntimeConsentRequest.provider_id'),
      requestId: _contractString(json['request_id'], 'RuntimeConsentRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'decision': decision.toJson(),
      'provider_id': providerId,
      'request_id': requestId,
    };
  }
}

class RuntimeConsentResponse {
  const RuntimeConsentResponse({
    required this.correlationId,
    required this.report,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final RuntimeHealthReport report;
  final RequestId requestId;

  factory RuntimeConsentResponse.fromJson(Map<String, dynamic> json) {
    return RuntimeConsentResponse(
      correlationId: _contractString(json['correlation_id'], 'RuntimeConsentResponse.correlation_id'),
      report: RuntimeHealthReport.fromJson(_contractMap(json['report'], 'RuntimeConsentResponse.report')),
      requestId: _contractString(json['request_id'], 'RuntimeConsentResponse.request_id'),
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

enum RuntimeConsentState {
  notRequested('not_requested'),
  reuseApproved('reuse_approved'),
  managementApproved('management_approved'),
  denied('denied'),
  unknown('unknown'),
  ;

  const RuntimeConsentState(this.wireValue);

  final String wireValue;

  static RuntimeConsentState fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

typedef RuntimeDisplayName = String;

enum RuntimeEndpointSafety {
  loopbackVerified('loopback_verified'),
  unsafe('unsafe'),
  unverified('unverified'),
  unknown('unknown'),
  ;

  const RuntimeEndpointSafety(this.wireValue);

  final String wireValue;

  static RuntimeEndpointSafety fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class RuntimeHealthReport {
  const RuntimeHealthReport({
    required this.capabilities,
    required this.displayName,
    required this.endpointSafety,
    required this.managementConsent,
    required this.ownership,
    required this.providerId,
    required this.reasons,
    required this.reuseConsent,
    required this.schemaVersion,
    required this.state,
    required this.version,
    required this.warnings,
  });

  final List<RuntimeCapabilityDescriptor> capabilities;
  final RuntimeDisplayName displayName;
  final RuntimeEndpointSafety endpointSafety;
  final RuntimeConsentState managementConsent;
  final RuntimeOwnership ownership;
  final RuntimeProviderId providerId;
  final List<RuntimeReason> reasons;
  final RuntimeConsentState reuseConsent;
  final int schemaVersion;
  final RuntimeState state;
  final RuntimeVersionInfo? version;
  final List<RuntimeWarning> warnings;

  factory RuntimeHealthReport.fromJson(Map<String, dynamic> json) {
    return RuntimeHealthReport(
      capabilities: _contractList(json['capabilities'], 'RuntimeHealthReport.capabilities').map((item) => RuntimeCapabilityDescriptor.fromJson(_contractMap(item, 'RuntimeHealthReport.capabilities[]'))).toList(growable: false),
      displayName: _contractString(json['display_name'], 'RuntimeHealthReport.display_name'),
      endpointSafety: RuntimeEndpointSafety.fromJson(json['endpoint_safety']),
      managementConsent: RuntimeConsentState.fromJson(json['management_consent']),
      ownership: RuntimeOwnership.fromJson(json['ownership']),
      providerId: _contractString(json['provider_id'], 'RuntimeHealthReport.provider_id'),
      reasons: _contractList(json['reasons'], 'RuntimeHealthReport.reasons').map((item) => RuntimeReason.fromJson(_contractMap(item, 'RuntimeHealthReport.reasons[]'))).toList(growable: false),
      reuseConsent: RuntimeConsentState.fromJson(json['reuse_consent']),
      schemaVersion: _contractInt(json['schema_version'], 'RuntimeHealthReport.schema_version'),
      state: RuntimeState.fromJson(json['state']),
      version: json['version'] == null ? null : RuntimeVersionInfo.fromJson(_contractMap(json['version'], 'RuntimeHealthReport.version')),
      warnings: _contractList(json['warnings'], 'RuntimeHealthReport.warnings').map((item) => RuntimeWarning.fromJson(_contractMap(item, 'RuntimeHealthReport.warnings[]'))).toList(growable: false),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'capabilities': capabilities.map((item) => item.toJson()).toList(growable: false),
      'display_name': displayName,
      'endpoint_safety': endpointSafety.toJson(),
      'management_consent': managementConsent.toJson(),
      'ownership': ownership.toJson(),
      'provider_id': providerId,
      'reasons': reasons.map((item) => item.toJson()).toList(growable: false),
      'reuse_consent': reuseConsent.toJson(),
      'schema_version': schemaVersion,
      'state': state.toJson(),
      'version': version?.toJson(),
      'warnings': warnings.map((item) => item.toJson()).toList(growable: false),
    };
  }
}

class RuntimeModelInventory {
  const RuntimeModelInventory({
    required this.collectedAtUnixMs,
    required this.models,
    required this.providerId,
    required this.schemaVersion,
    required this.truncated,
  });

  final int collectedAtUnixMs;
  final List<RuntimeModelSummary> models;
  final RuntimeProviderId providerId;
  final int schemaVersion;
  final bool truncated;

  factory RuntimeModelInventory.fromJson(Map<String, dynamic> json) {
    return RuntimeModelInventory(
      collectedAtUnixMs: _contractInt(json['collected_at_unix_ms'], 'RuntimeModelInventory.collected_at_unix_ms'),
      models: _contractList(json['models'], 'RuntimeModelInventory.models').map((item) => RuntimeModelSummary.fromJson(_contractMap(item, 'RuntimeModelInventory.models[]'))).toList(growable: false),
      providerId: _contractString(json['provider_id'], 'RuntimeModelInventory.provider_id'),
      schemaVersion: _contractInt(json['schema_version'], 'RuntimeModelInventory.schema_version'),
      truncated: _contractBool(json['truncated'], 'RuntimeModelInventory.truncated'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'collected_at_unix_ms': collectedAtUnixMs,
      'models': models.map((item) => item.toJson()).toList(growable: false),
      'provider_id': providerId,
      'schema_version': schemaVersion,
      'truncated': truncated,
    };
  }
}

class RuntimeModelInventoryRequest {
  const RuntimeModelInventoryRequest({
    required this.correlationId,
    required this.limit,
    required this.providerId,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final int limit;
  final RuntimeProviderId providerId;
  final RequestId requestId;

  factory RuntimeModelInventoryRequest.fromJson(Map<String, dynamic> json) {
    return RuntimeModelInventoryRequest(
      correlationId: _contractString(json['correlation_id'], 'RuntimeModelInventoryRequest.correlation_id'),
      limit: _contractInt(json['limit'], 'RuntimeModelInventoryRequest.limit'),
      providerId: _contractString(json['provider_id'], 'RuntimeModelInventoryRequest.provider_id'),
      requestId: _contractString(json['request_id'], 'RuntimeModelInventoryRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'limit': limit,
      'provider_id': providerId,
      'request_id': requestId,
    };
  }
}

class RuntimeModelInventoryResponse {
  const RuntimeModelInventoryResponse({
    required this.correlationId,
    required this.inventory,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final RuntimeModelInventory inventory;
  final RequestId requestId;

  factory RuntimeModelInventoryResponse.fromJson(Map<String, dynamic> json) {
    return RuntimeModelInventoryResponse(
      correlationId: _contractString(json['correlation_id'], 'RuntimeModelInventoryResponse.correlation_id'),
      inventory: RuntimeModelInventory.fromJson(_contractMap(json['inventory'], 'RuntimeModelInventoryResponse.inventory')),
      requestId: _contractString(json['request_id'], 'RuntimeModelInventoryResponse.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'inventory': inventory.toJson(),
      'request_id': requestId,
    };
  }
}

enum RuntimeModelMappingStatus {
  matched('matched'),
  external('external'),
  unknown('unknown'),
  ;

  const RuntimeModelMappingStatus(this.wireValue);

  final String wireValue;

  static RuntimeModelMappingStatus fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class RuntimeModelSummary {
  const RuntimeModelSummary({
    required this.displayName,
    required this.mapping,
    required this.providerModelId,
    required this.sizeBytes,
  });

  final String displayName;
  final RuntimeProviderModelMapping mapping;
  final RuntimeProviderModelId providerModelId;
  final int? sizeBytes;

  factory RuntimeModelSummary.fromJson(Map<String, dynamic> json) {
    return RuntimeModelSummary(
      displayName: _contractString(json['display_name'], 'RuntimeModelSummary.display_name'),
      mapping: RuntimeProviderModelMapping.fromJson(_contractMap(json['mapping'], 'RuntimeModelSummary.mapping')),
      providerModelId: _contractString(json['provider_model_id'], 'RuntimeModelSummary.provider_model_id'),
      sizeBytes: json['size_bytes'] == null ? null : _contractInt(json['size_bytes'], 'RuntimeModelSummary.size_bytes'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'display_name': displayName,
      'mapping': mapping.toJson(),
      'provider_model_id': providerModelId,
      'size_bytes': sizeBytes,
    };
  }
}

class RuntimeOperationEvent {
  const RuntimeOperationEvent({
    required this.correlationId,
    required this.error,
    required this.kind,
    required this.message,
    required this.operationId,
    required this.operationKind,
    required this.report,
    required this.schemaVersion,
    required this.sequence,
    required this.terminalState,
    required this.timestampUnixMs,
  });

  final CorrelationId correlationId;
  final SafeErrorPayload? error;
  final RuntimeOperationEventKind kind;
  final String? message;
  final OperationId operationId;
  final RuntimeOperationKind operationKind;
  final RuntimeHealthReport? report;
  final int schemaVersion;
  final int sequence;
  final RuntimeOperationTerminalState? terminalState;
  final int timestampUnixMs;

  factory RuntimeOperationEvent.fromJson(Map<String, dynamic> json) {
    return RuntimeOperationEvent(
      correlationId: _contractString(json['correlation_id'], 'RuntimeOperationEvent.correlation_id'),
      error: json['error'] == null ? null : SafeErrorPayload.fromJson(_contractMap(json['error'], 'RuntimeOperationEvent.error')),
      kind: RuntimeOperationEventKind.fromJson(json['kind']),
      message: json['message'] == null ? null : _contractString(json['message'], 'RuntimeOperationEvent.message'),
      operationId: _contractString(json['operation_id'], 'RuntimeOperationEvent.operation_id'),
      operationKind: RuntimeOperationKind.fromJson(json['operation_kind']),
      report: json['report'] == null ? null : RuntimeHealthReport.fromJson(_contractMap(json['report'], 'RuntimeOperationEvent.report')),
      schemaVersion: _contractInt(json['schema_version'], 'RuntimeOperationEvent.schema_version'),
      sequence: _contractInt(json['sequence'], 'RuntimeOperationEvent.sequence'),
      terminalState: json['terminal_state'] == null ? null : RuntimeOperationTerminalState.fromJson(json['terminal_state']),
      timestampUnixMs: _contractInt(json['timestamp_unix_ms'], 'RuntimeOperationEvent.timestamp_unix_ms'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'error': error?.toJson(),
      'kind': kind.toJson(),
      'message': message,
      'operation_id': operationId,
      'operation_kind': operationKind.toJson(),
      'report': report?.toJson(),
      'schema_version': schemaVersion,
      'sequence': sequence,
      'terminal_state': terminalState?.toJson(),
      'timestamp_unix_ms': timestampUnixMs,
    };
  }
}

enum RuntimeOperationEventKind {
  started('started'),
  progress('progress'),
  completed('completed'),
  cancelled('cancelled'),
  failed('failed'),
  timedOut('timed_out'),
  unknown('unknown'),
  ;

  const RuntimeOperationEventKind(this.wireValue);

  final String wireValue;

  static RuntimeOperationEventKind fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

enum RuntimeOperationKind {
  start('start'),
  stop('stop'),
  restart('restart'),
  unknown('unknown'),
  ;

  const RuntimeOperationKind(this.wireValue);

  final String wireValue;

  static RuntimeOperationKind fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class RuntimeOperationStartRequest {
  const RuntimeOperationStartRequest({
    required this.correlationId,
    required this.kind,
    required this.providerId,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final RuntimeOperationKind kind;
  final RuntimeProviderId providerId;
  final RequestId requestId;

  factory RuntimeOperationStartRequest.fromJson(Map<String, dynamic> json) {
    return RuntimeOperationStartRequest(
      correlationId: _contractString(json['correlation_id'], 'RuntimeOperationStartRequest.correlation_id'),
      kind: RuntimeOperationKind.fromJson(json['kind']),
      providerId: _contractString(json['provider_id'], 'RuntimeOperationStartRequest.provider_id'),
      requestId: _contractString(json['request_id'], 'RuntimeOperationStartRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'kind': kind.toJson(),
      'provider_id': providerId,
      'request_id': requestId,
    };
  }
}

class RuntimeOperationStartResponse {
  const RuntimeOperationStartResponse({
    required this.correlationId,
    required this.operationId,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final OperationId operationId;
  final RequestId requestId;

  factory RuntimeOperationStartResponse.fromJson(Map<String, dynamic> json) {
    return RuntimeOperationStartResponse(
      correlationId: _contractString(json['correlation_id'], 'RuntimeOperationStartResponse.correlation_id'),
      operationId: _contractString(json['operation_id'], 'RuntimeOperationStartResponse.operation_id'),
      requestId: _contractString(json['request_id'], 'RuntimeOperationStartResponse.request_id'),
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

enum RuntimeOperationTerminalState {
  completed('completed'),
  cancelled('cancelled'),
  failed('failed'),
  timedOut('timed_out'),
  unknown('unknown'),
  ;

  const RuntimeOperationTerminalState(this.wireValue);

  final String wireValue;

  static RuntimeOperationTerminalState fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

enum RuntimeOwnership {
  external('external'),
  gixGizManaged('gix_giz_managed'),
  bundled('bundled'),
  unknown('unknown'),
  ;

  const RuntimeOwnership(this.wireValue);

  final String wireValue;

  static RuntimeOwnership fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

typedef RuntimeProviderId = String;

typedef RuntimeProviderModelId = String;

class RuntimeProviderModelMapping {
  const RuntimeProviderModelMapping({
    required this.catalogueId,
    required this.status,
  });

  final CandidateModelId? catalogueId;
  final RuntimeModelMappingStatus status;

  factory RuntimeProviderModelMapping.fromJson(Map<String, dynamic> json) {
    return RuntimeProviderModelMapping(
      catalogueId: json['catalogue_id'] == null ? null : _contractString(json['catalogue_id'], 'RuntimeProviderModelMapping.catalogue_id'),
      status: RuntimeModelMappingStatus.fromJson(json['status']),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'catalogue_id': catalogueId,
      'status': status.toJson(),
    };
  }
}

class RuntimeReason {
  const RuntimeReason({
    required this.code,
    required this.message,
  });

  final RuntimeReasonCode code;
  final String message;

  factory RuntimeReason.fromJson(Map<String, dynamic> json) {
    return RuntimeReason(
      code: RuntimeReasonCode.fromJson(json['code']),
      message: _contractString(json['message'], 'RuntimeReason.message'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'code': code.toJson(),
      'message': message,
    };
  }
}

enum RuntimeReasonCode {
  installationNotFound('installation_not_found'),
  executableVerified('executable_verified'),
  endpointUnavailable('endpoint_unavailable'),
  endpointReachable('endpoint_reachable'),
  endpointUnsafe('endpoint_unsafe'),
  versionCompatible('version_compatible'),
  versionIncompatible('version_incompatible'),
  versionUnverified('version_unverified'),
  evidenceIncomplete('evidence_incomplete'),
  ownershipRequired('ownership_required'),
  consentRequired('consent_required'),
  operationUnsupported('operation_unsupported'),
  processExited('process_exited'),
  unknown('unknown'),
  ;

  const RuntimeReasonCode(this.wireValue);

  final String wireValue;

  static RuntimeReasonCode fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

enum RuntimeState {
  notInstalled('not_installed'),
  installedStopped('installed_stopped'),
  starting('starting'),
  ready('ready'),
  degraded('degraded'),
  incompatible('incompatible'),
  updating('updating'),
  failed('failed'),
  unknown('unknown'),
  ;

  const RuntimeState(this.wireValue);

  final String wireValue;

  static RuntimeState fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class RuntimeStatusRequest {
  const RuntimeStatusRequest({
    required this.correlationId,
    required this.providerId,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final RuntimeProviderId providerId;
  final RequestId requestId;

  factory RuntimeStatusRequest.fromJson(Map<String, dynamic> json) {
    return RuntimeStatusRequest(
      correlationId: _contractString(json['correlation_id'], 'RuntimeStatusRequest.correlation_id'),
      providerId: _contractString(json['provider_id'], 'RuntimeStatusRequest.provider_id'),
      requestId: _contractString(json['request_id'], 'RuntimeStatusRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'provider_id': providerId,
      'request_id': requestId,
    };
  }
}

class RuntimeStatusResponse {
  const RuntimeStatusResponse({
    required this.correlationId,
    required this.report,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final RuntimeHealthReport report;
  final RequestId requestId;

  factory RuntimeStatusResponse.fromJson(Map<String, dynamic> json) {
    return RuntimeStatusResponse(
      correlationId: _contractString(json['correlation_id'], 'RuntimeStatusResponse.correlation_id'),
      report: RuntimeHealthReport.fromJson(_contractMap(json['report'], 'RuntimeStatusResponse.report')),
      requestId: _contractString(json['request_id'], 'RuntimeStatusResponse.request_id'),
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

enum RuntimeVersionCompatibility {
  compatible('compatible'),
  incompatible('incompatible'),
  untested('untested'),
  unknown('unknown'),
  ;

  const RuntimeVersionCompatibility(this.wireValue);

  final String wireValue;

  static RuntimeVersionCompatibility fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class RuntimeVersionInfo {
  const RuntimeVersionInfo({
    required this.compatibility,
    required this.normalizedVersion,
    required this.reportedVersion,
  });

  final RuntimeVersionCompatibility compatibility;
  final String? normalizedVersion;
  final String reportedVersion;

  factory RuntimeVersionInfo.fromJson(Map<String, dynamic> json) {
    return RuntimeVersionInfo(
      compatibility: RuntimeVersionCompatibility.fromJson(json['compatibility']),
      normalizedVersion: json['normalized_version'] == null ? null : _contractString(json['normalized_version'], 'RuntimeVersionInfo.normalized_version'),
      reportedVersion: _contractString(json['reported_version'], 'RuntimeVersionInfo.reported_version'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'compatibility': compatibility.toJson(),
      'normalized_version': normalizedVersion,
      'reported_version': reportedVersion,
    };
  }
}

class RuntimeWarning {
  const RuntimeWarning({
    required this.code,
    required this.message,
  });

  final RuntimeWarningCode code;
  final String message;

  factory RuntimeWarning.fromJson(Map<String, dynamic> json) {
    return RuntimeWarning(
      code: RuntimeWarningCode.fromJson(json['code']),
      message: _contractString(json['message'], 'RuntimeWarning.message'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'code': code.toJson(),
      'message': message,
    };
  }
}

enum RuntimeWarningCode {
  externalInstallation('external_installation'),
  endpointExposure('endpoint_exposure'),
  versionUntested('version_untested'),
  partialEvidence('partial_evidence'),
  modelInventoryTruncated('model_inventory_truncated'),
  unknown('unknown'),
  ;

  const RuntimeWarningCode(this.wireValue);

  final String wireValue;

  static RuntimeWarningCode fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

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

class SendMessageRequest {
  const SendMessageRequest({
    required this.content,
    required this.conversationId,
    required this.correlationId,
    required this.requestId,
  });

  final String content;
  final ConversationId conversationId;
  final CorrelationId correlationId;
  final RequestId requestId;

  factory SendMessageRequest.fromJson(Map<String, dynamic> json) {
    return SendMessageRequest(
      content: _contractString(json['content'], 'SendMessageRequest.content'),
      conversationId: _contractString(json['conversation_id'], 'SendMessageRequest.conversation_id'),
      correlationId: _contractString(json['correlation_id'], 'SendMessageRequest.correlation_id'),
      requestId: _contractString(json['request_id'], 'SendMessageRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'content': content,
      'conversation_id': conversationId,
      'correlation_id': correlationId,
      'request_id': requestId,
    };
  }
}

class SendMessageResponse {
  const SendMessageResponse({
    required this.assistantMessage,
    required this.correlationId,
    required this.generationId,
    required this.requestId,
    required this.schemaVersion,
    required this.userMessage,
    required this.warnings,
  });

  final ChatMessage assistantMessage;
  final CorrelationId correlationId;
  final GenerationId generationId;
  final RequestId requestId;
  final int schemaVersion;
  final ChatMessage userMessage;
  final List<ChatWarning> warnings;

  factory SendMessageResponse.fromJson(Map<String, dynamic> json) {
    return SendMessageResponse(
      assistantMessage: ChatMessage.fromJson(_contractMap(json['assistant_message'], 'SendMessageResponse.assistant_message')),
      correlationId: _contractString(json['correlation_id'], 'SendMessageResponse.correlation_id'),
      generationId: _contractString(json['generation_id'], 'SendMessageResponse.generation_id'),
      requestId: _contractString(json['request_id'], 'SendMessageResponse.request_id'),
      schemaVersion: _contractInt(json['schema_version'], 'SendMessageResponse.schema_version'),
      userMessage: ChatMessage.fromJson(_contractMap(json['user_message'], 'SendMessageResponse.user_message')),
      warnings: _contractList(json['warnings'], 'SendMessageResponse.warnings').map((item) => ChatWarning.fromJson(_contractMap(item, 'SendMessageResponse.warnings[]'))).toList(growable: false),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'assistant_message': assistantMessage.toJson(),
      'correlation_id': correlationId,
      'generation_id': generationId,
      'request_id': requestId,
      'schema_version': schemaVersion,
      'user_message': userMessage.toJson(),
      'warnings': warnings.map((item) => item.toJson()).toList(growable: false),
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

enum SetupApprovalDecision {
  approve('approve'),
  deny('deny'),
  unknown('unknown'),
  ;

  const SetupApprovalDecision(this.wireValue);

  final String wireValue;

  static SetupApprovalDecision fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class SetupApprovalRecord {
  const SetupApprovalRecord({
    required this.approvedEffects,
    required this.canonicalModelId,
    required this.correlationId,
    required this.decidedAtUnixMs,
    required this.decision,
    required this.destination,
    required this.expectedSizeBytes,
    required this.externalRuntimeEffect,
    required this.jobId,
    required this.licenceSpdx,
    required this.planRevision,
    required this.provenance,
    required this.providerId,
    required this.providerModelId,
    required this.requestId,
  });

  final List<SetupEffectKind> approvedEffects;
  final CandidateModelId canonicalModelId;
  final CorrelationId correlationId;
  final int decidedAtUnixMs;
  final SetupApprovalDecision decision;
  final SetupDestinationCategory destination;
  final int expectedSizeBytes;
  final bool externalRuntimeEffect;
  final SetupJobId jobId;
  final String licenceSpdx;
  final int planRevision;
  final String provenance;
  final RuntimeProviderId providerId;
  final RuntimeProviderModelId providerModelId;
  final RequestId requestId;

  factory SetupApprovalRecord.fromJson(Map<String, dynamic> json) {
    return SetupApprovalRecord(
      approvedEffects: _contractList(json['approved_effects'], 'SetupApprovalRecord.approved_effects').map((item) => SetupEffectKind.fromJson(item)).toList(growable: false),
      canonicalModelId: _contractString(json['canonical_model_id'], 'SetupApprovalRecord.canonical_model_id'),
      correlationId: _contractString(json['correlation_id'], 'SetupApprovalRecord.correlation_id'),
      decidedAtUnixMs: _contractInt(json['decided_at_unix_ms'], 'SetupApprovalRecord.decided_at_unix_ms'),
      decision: SetupApprovalDecision.fromJson(json['decision']),
      destination: SetupDestinationCategory.fromJson(json['destination']),
      expectedSizeBytes: _contractInt(json['expected_size_bytes'], 'SetupApprovalRecord.expected_size_bytes'),
      externalRuntimeEffect: _contractBool(json['external_runtime_effect'], 'SetupApprovalRecord.external_runtime_effect'),
      jobId: _contractString(json['job_id'], 'SetupApprovalRecord.job_id'),
      licenceSpdx: _contractString(json['licence_spdx'], 'SetupApprovalRecord.licence_spdx'),
      planRevision: _contractInt(json['plan_revision'], 'SetupApprovalRecord.plan_revision'),
      provenance: _contractString(json['provenance'], 'SetupApprovalRecord.provenance'),
      providerId: _contractString(json['provider_id'], 'SetupApprovalRecord.provider_id'),
      providerModelId: _contractString(json['provider_model_id'], 'SetupApprovalRecord.provider_model_id'),
      requestId: _contractString(json['request_id'], 'SetupApprovalRecord.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'approved_effects': approvedEffects.map((item) => item.toJson()).toList(growable: false),
      'canonical_model_id': canonicalModelId,
      'correlation_id': correlationId,
      'decided_at_unix_ms': decidedAtUnixMs,
      'decision': decision.toJson(),
      'destination': destination.toJson(),
      'expected_size_bytes': expectedSizeBytes,
      'external_runtime_effect': externalRuntimeEffect,
      'job_id': jobId,
      'licence_spdx': licenceSpdx,
      'plan_revision': planRevision,
      'provenance': provenance,
      'provider_id': providerId,
      'provider_model_id': providerModelId,
      'request_id': requestId,
    };
  }
}

class SetupApprovalRequest {
  const SetupApprovalRequest({
    required this.correlationId,
    required this.decision,
    required this.jobId,
    required this.planRevision,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final SetupApprovalDecision decision;
  final SetupJobId jobId;
  final int planRevision;
  final RequestId requestId;

  factory SetupApprovalRequest.fromJson(Map<String, dynamic> json) {
    return SetupApprovalRequest(
      correlationId: _contractString(json['correlation_id'], 'SetupApprovalRequest.correlation_id'),
      decision: SetupApprovalDecision.fromJson(json['decision']),
      jobId: _contractString(json['job_id'], 'SetupApprovalRequest.job_id'),
      planRevision: _contractInt(json['plan_revision'], 'SetupApprovalRequest.plan_revision'),
      requestId: _contractString(json['request_id'], 'SetupApprovalRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'decision': decision.toJson(),
      'job_id': jobId,
      'plan_revision': planRevision,
      'request_id': requestId,
    };
  }
}

class SetupApprovalResponse {
  const SetupApprovalResponse({
    required this.approval,
    required this.correlationId,
    required this.job,
    required this.requestId,
  });

  final SetupApprovalRecord approval;
  final CorrelationId correlationId;
  final SetupJobSnapshot job;
  final RequestId requestId;

  factory SetupApprovalResponse.fromJson(Map<String, dynamic> json) {
    return SetupApprovalResponse(
      approval: SetupApprovalRecord.fromJson(_contractMap(json['approval'], 'SetupApprovalResponse.approval')),
      correlationId: _contractString(json['correlation_id'], 'SetupApprovalResponse.correlation_id'),
      job: SetupJobSnapshot.fromJson(_contractMap(json['job'], 'SetupApprovalResponse.job')),
      requestId: _contractString(json['request_id'], 'SetupApprovalResponse.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'approval': approval.toJson(),
      'correlation_id': correlationId,
      'job': job.toJson(),
      'request_id': requestId,
    };
  }
}

enum SetupAttentionReason {
  runtimeNotInstalled('runtime_not_installed'),
  runtimeConsentRequired('runtime_consent_required'),
  runtimeUnavailable('runtime_unavailable'),
  privilegedRuntimeInstallationRequired('privileged_runtime_installation_required'),
  destinationUnavailable('destination_unavailable'),
  insufficientStorage('insufficient_storage'),
  acquisitionInterrupted('acquisition_interrupted'),
  registrationUnverified('registration_unverified'),
  modelUnavailable('model_unavailable'),
  integrityMismatch('integrity_mismatch'),
  readinessTimedOut('readiness_timed_out'),
  readinessFailed('readiness_failed'),
  recoveryRequired('recovery_required'),
  unknown('unknown'),
  ;

  const SetupAttentionReason(this.wireValue);

  final String wireValue;

  static SetupAttentionReason fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class SetupCancellationReport {
  const SetupCancellationReport({
    required this.effectReport,
    required this.observedAtUnixMs,
    required this.requestedAtUnixMs,
  });

  final SetupEffectReport effectReport;
  final int observedAtUnixMs;
  final int requestedAtUnixMs;

  factory SetupCancellationReport.fromJson(Map<String, dynamic> json) {
    return SetupCancellationReport(
      effectReport: SetupEffectReport.fromJson(_contractMap(json['effect_report'], 'SetupCancellationReport.effect_report')),
      observedAtUnixMs: _contractInt(json['observed_at_unix_ms'], 'SetupCancellationReport.observed_at_unix_ms'),
      requestedAtUnixMs: _contractInt(json['requested_at_unix_ms'], 'SetupCancellationReport.requested_at_unix_ms'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'effect_report': effectReport.toJson(),
      'observed_at_unix_ms': observedAtUnixMs,
      'requested_at_unix_ms': requestedAtUnixMs,
    };
  }
}

enum SetupDestinationCategory {
  providerManaged('provider_managed'),
  applicationData('application_data'),
  userSelected('user_selected'),
  unknown('unknown'),
  ;

  const SetupDestinationCategory(this.wireValue);

  final String wireValue;

  static SetupDestinationCategory fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class SetupEffect {
  const SetupEffect({
    required this.disposition,
    required this.kind,
    required this.message,
  });

  final SetupEffectDisposition disposition;
  final SetupEffectKind kind;
  final String message;

  factory SetupEffect.fromJson(Map<String, dynamic> json) {
    return SetupEffect(
      disposition: SetupEffectDisposition.fromJson(json['disposition']),
      kind: SetupEffectKind.fromJson(json['kind']),
      message: _contractString(json['message'], 'SetupEffect.message'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'disposition': disposition.toJson(),
      'kind': kind.toJson(),
      'message': message,
    };
  }
}

enum SetupEffectDisposition {
  completed('completed'),
  retained('retained'),
  rolledBack('rolled_back'),
  uncertain('uncertain'),
  unknown('unknown'),
  ;

  const SetupEffectDisposition(this.wireValue);

  final String wireValue;

  static SetupEffectDisposition fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

enum SetupEffectKind {
  providerModelAcquisition('provider_model_acquisition'),
  providerModelRegistration('provider_model_registration'),
  readinessInference('readiness_inference'),
  metadataPersistence('metadata_persistence'),
  unknown('unknown'),
  ;

  const SetupEffectKind(this.wireValue);

  final String wireValue;

  static SetupEffectKind fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class SetupEffectReport {
  const SetupEffectReport({
    required this.effects,
  });

  final List<SetupEffect> effects;

  factory SetupEffectReport.fromJson(Map<String, dynamic> json) {
    return SetupEffectReport(
      effects: _contractList(json['effects'], 'SetupEffectReport.effects').map((item) => SetupEffect.fromJson(_contractMap(item, 'SetupEffectReport.effects[]'))).toList(growable: false),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'effects': effects.map((item) => item.toJson()).toList(growable: false),
    };
  }
}

class SetupJobCancelRequest {
  const SetupJobCancelRequest({
    required this.correlationId,
    required this.jobId,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final SetupJobId jobId;
  final RequestId requestId;

  factory SetupJobCancelRequest.fromJson(Map<String, dynamic> json) {
    return SetupJobCancelRequest(
      correlationId: _contractString(json['correlation_id'], 'SetupJobCancelRequest.correlation_id'),
      jobId: _contractString(json['job_id'], 'SetupJobCancelRequest.job_id'),
      requestId: _contractString(json['request_id'], 'SetupJobCancelRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'job_id': jobId,
      'request_id': requestId,
    };
  }
}

class SetupJobCancelResponse {
  const SetupJobCancelResponse({
    required this.accepted,
    required this.correlationId,
    required this.job,
    required this.requestId,
  });

  final bool accepted;
  final CorrelationId correlationId;
  final SetupJobSnapshot job;
  final RequestId requestId;

  factory SetupJobCancelResponse.fromJson(Map<String, dynamic> json) {
    return SetupJobCancelResponse(
      accepted: _contractBool(json['accepted'], 'SetupJobCancelResponse.accepted'),
      correlationId: _contractString(json['correlation_id'], 'SetupJobCancelResponse.correlation_id'),
      job: SetupJobSnapshot.fromJson(_contractMap(json['job'], 'SetupJobCancelResponse.job')),
      requestId: _contractString(json['request_id'], 'SetupJobCancelResponse.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'accepted': accepted,
      'correlation_id': correlationId,
      'job': job.toJson(),
      'request_id': requestId,
    };
  }
}

class SetupJobEvent {
  const SetupJobEvent({
    required this.correlationId,
    required this.job,
    required this.jobId,
    required this.kind,
    required this.schemaVersion,
    required this.sequence,
    required this.terminalState,
    required this.timestampUnixMs,
  });

  final CorrelationId correlationId;
  final SetupJobSnapshot job;
  final SetupJobId jobId;
  final SetupJobEventKind kind;
  final int schemaVersion;
  final int sequence;
  final SetupJobTerminalState? terminalState;
  final int timestampUnixMs;

  factory SetupJobEvent.fromJson(Map<String, dynamic> json) {
    return SetupJobEvent(
      correlationId: _contractString(json['correlation_id'], 'SetupJobEvent.correlation_id'),
      job: SetupJobSnapshot.fromJson(_contractMap(json['job'], 'SetupJobEvent.job')),
      jobId: _contractString(json['job_id'], 'SetupJobEvent.job_id'),
      kind: SetupJobEventKind.fromJson(json['kind']),
      schemaVersion: _contractInt(json['schema_version'], 'SetupJobEvent.schema_version'),
      sequence: _contractInt(json['sequence'], 'SetupJobEvent.sequence'),
      terminalState: json['terminal_state'] == null ? null : SetupJobTerminalState.fromJson(json['terminal_state']),
      timestampUnixMs: _contractInt(json['timestamp_unix_ms'], 'SetupJobEvent.timestamp_unix_ms'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'job': job.toJson(),
      'job_id': jobId,
      'kind': kind.toJson(),
      'schema_version': schemaVersion,
      'sequence': sequence,
      'terminal_state': terminalState?.toJson(),
      'timestamp_unix_ms': timestampUnixMs,
    };
  }
}

enum SetupJobEventKind {
  started('started'),
  progress('progress'),
  attentionRequired('attention_required'),
  ready('ready'),
  failed('failed'),
  cancelled('cancelled'),
  unknown('unknown'),
  ;

  const SetupJobEventKind(this.wireValue);

  final String wireValue;

  static SetupJobEventKind fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class SetupJobEventsRequest {
  const SetupJobEventsRequest({
    required this.afterSequence,
    required this.correlationId,
    required this.jobId,
    required this.limit,
    required this.requestId,
  });

  final int afterSequence;
  final CorrelationId correlationId;
  final SetupJobId jobId;
  final int limit;
  final RequestId requestId;

  factory SetupJobEventsRequest.fromJson(Map<String, dynamic> json) {
    return SetupJobEventsRequest(
      afterSequence: _contractInt(json['after_sequence'], 'SetupJobEventsRequest.after_sequence'),
      correlationId: _contractString(json['correlation_id'], 'SetupJobEventsRequest.correlation_id'),
      jobId: _contractString(json['job_id'], 'SetupJobEventsRequest.job_id'),
      limit: _contractInt(json['limit'], 'SetupJobEventsRequest.limit'),
      requestId: _contractString(json['request_id'], 'SetupJobEventsRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'after_sequence': afterSequence,
      'correlation_id': correlationId,
      'job_id': jobId,
      'limit': limit,
      'request_id': requestId,
    };
  }
}

class SetupJobEventsResponse {
  const SetupJobEventsResponse({
    required this.correlationId,
    required this.events,
    required this.hasMore,
    required this.nextAfterSequence,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final List<SetupJobEvent> events;
  final bool hasMore;
  final int nextAfterSequence;
  final RequestId requestId;

  factory SetupJobEventsResponse.fromJson(Map<String, dynamic> json) {
    return SetupJobEventsResponse(
      correlationId: _contractString(json['correlation_id'], 'SetupJobEventsResponse.correlation_id'),
      events: _contractList(json['events'], 'SetupJobEventsResponse.events').map((item) => SetupJobEvent.fromJson(_contractMap(item, 'SetupJobEventsResponse.events[]'))).toList(growable: false),
      hasMore: _contractBool(json['has_more'], 'SetupJobEventsResponse.has_more'),
      nextAfterSequence: _contractInt(json['next_after_sequence'], 'SetupJobEventsResponse.next_after_sequence'),
      requestId: _contractString(json['request_id'], 'SetupJobEventsResponse.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'events': events.map((item) => item.toJson()).toList(growable: false),
      'has_more': hasMore,
      'next_after_sequence': nextAfterSequence,
      'request_id': requestId,
    };
  }
}

typedef SetupJobId = String;

class SetupJobRecoveryRequest {
  const SetupJobRecoveryRequest({
    required this.correlationId,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final RequestId requestId;

  factory SetupJobRecoveryRequest.fromJson(Map<String, dynamic> json) {
    return SetupJobRecoveryRequest(
      correlationId: _contractString(json['correlation_id'], 'SetupJobRecoveryRequest.correlation_id'),
      requestId: _contractString(json['request_id'], 'SetupJobRecoveryRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'request_id': requestId,
    };
  }
}

class SetupJobRecoveryResponse {
  const SetupJobRecoveryResponse({
    required this.correlationId,
    required this.job,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final SetupJobSnapshot? job;
  final RequestId requestId;

  factory SetupJobRecoveryResponse.fromJson(Map<String, dynamic> json) {
    return SetupJobRecoveryResponse(
      correlationId: _contractString(json['correlation_id'], 'SetupJobRecoveryResponse.correlation_id'),
      job: json['job'] == null ? null : SetupJobSnapshot.fromJson(_contractMap(json['job'], 'SetupJobRecoveryResponse.job')),
      requestId: _contractString(json['request_id'], 'SetupJobRecoveryResponse.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'job': job?.toJson(),
      'request_id': requestId,
    };
  }
}

class SetupJobRetryRequest {
  const SetupJobRetryRequest({
    required this.correlationId,
    required this.jobId,
    required this.planRevision,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final SetupJobId jobId;
  final int planRevision;
  final RequestId requestId;

  factory SetupJobRetryRequest.fromJson(Map<String, dynamic> json) {
    return SetupJobRetryRequest(
      correlationId: _contractString(json['correlation_id'], 'SetupJobRetryRequest.correlation_id'),
      jobId: _contractString(json['job_id'], 'SetupJobRetryRequest.job_id'),
      planRevision: _contractInt(json['plan_revision'], 'SetupJobRetryRequest.plan_revision'),
      requestId: _contractString(json['request_id'], 'SetupJobRetryRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'job_id': jobId,
      'plan_revision': planRevision,
      'request_id': requestId,
    };
  }
}

class SetupJobRetryResponse {
  const SetupJobRetryResponse({
    required this.correlationId,
    required this.job,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final SetupJobSnapshot job;
  final RequestId requestId;

  factory SetupJobRetryResponse.fromJson(Map<String, dynamic> json) {
    return SetupJobRetryResponse(
      correlationId: _contractString(json['correlation_id'], 'SetupJobRetryResponse.correlation_id'),
      job: SetupJobSnapshot.fromJson(_contractMap(json['job'], 'SetupJobRetryResponse.job')),
      requestId: _contractString(json['request_id'], 'SetupJobRetryResponse.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'job': job.toJson(),
      'request_id': requestId,
    };
  }
}

class SetupJobSnapshot {
  const SetupJobSnapshot({
    required this.attentionReason,
    required this.cancellationReport,
    required this.cancellationRequested,
    required this.createdAtUnixMs,
    required this.effectReport,
    required this.error,
    required this.jobId,
    required this.latestEventSequence,
    required this.plan,
    required this.progress,
    required this.recoveryAction,
    required this.retryCount,
    required this.schemaVersion,
    required this.stage,
    required this.state,
    required this.updatedAtUnixMs,
  });

  final SetupAttentionReason? attentionReason;
  final SetupCancellationReport? cancellationReport;
  final bool cancellationRequested;
  final int createdAtUnixMs;
  final SetupEffectReport? effectReport;
  final SafeErrorPayload? error;
  final SetupJobId jobId;
  final int latestEventSequence;
  final SetupPlan plan;
  final ModelAcquisitionProgress? progress;
  final SetupRecoveryAction? recoveryAction;
  final int retryCount;
  final int schemaVersion;
  final SetupStage stage;
  final SetupJobState state;
  final int updatedAtUnixMs;

  factory SetupJobSnapshot.fromJson(Map<String, dynamic> json) {
    return SetupJobSnapshot(
      attentionReason: json['attention_reason'] == null ? null : SetupAttentionReason.fromJson(json['attention_reason']),
      cancellationReport: json['cancellation_report'] == null ? null : SetupCancellationReport.fromJson(_contractMap(json['cancellation_report'], 'SetupJobSnapshot.cancellation_report')),
      cancellationRequested: _contractBool(json['cancellation_requested'], 'SetupJobSnapshot.cancellation_requested'),
      createdAtUnixMs: _contractInt(json['created_at_unix_ms'], 'SetupJobSnapshot.created_at_unix_ms'),
      effectReport: json['effect_report'] == null ? null : SetupEffectReport.fromJson(_contractMap(json['effect_report'], 'SetupJobSnapshot.effect_report')),
      error: json['error'] == null ? null : SafeErrorPayload.fromJson(_contractMap(json['error'], 'SetupJobSnapshot.error')),
      jobId: _contractString(json['job_id'], 'SetupJobSnapshot.job_id'),
      latestEventSequence: _contractInt(json['latest_event_sequence'], 'SetupJobSnapshot.latest_event_sequence'),
      plan: SetupPlan.fromJson(_contractMap(json['plan'], 'SetupJobSnapshot.plan')),
      progress: json['progress'] == null ? null : ModelAcquisitionProgress.fromJson(_contractMap(json['progress'], 'SetupJobSnapshot.progress')),
      recoveryAction: json['recovery_action'] == null ? null : SetupRecoveryAction.fromJson(json['recovery_action']),
      retryCount: _contractInt(json['retry_count'], 'SetupJobSnapshot.retry_count'),
      schemaVersion: _contractInt(json['schema_version'], 'SetupJobSnapshot.schema_version'),
      stage: SetupStage.fromJson(json['stage']),
      state: SetupJobState.fromJson(json['state']),
      updatedAtUnixMs: _contractInt(json['updated_at_unix_ms'], 'SetupJobSnapshot.updated_at_unix_ms'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'attention_reason': attentionReason?.toJson(),
      'cancellation_report': cancellationReport?.toJson(),
      'cancellation_requested': cancellationRequested,
      'created_at_unix_ms': createdAtUnixMs,
      'effect_report': effectReport?.toJson(),
      'error': error?.toJson(),
      'job_id': jobId,
      'latest_event_sequence': latestEventSequence,
      'plan': plan.toJson(),
      'progress': progress?.toJson(),
      'recovery_action': recoveryAction?.toJson(),
      'retry_count': retryCount,
      'schema_version': schemaVersion,
      'stage': stage.toJson(),
      'state': state.toJson(),
      'updated_at_unix_ms': updatedAtUnixMs,
    };
  }
}

class SetupJobStartRequest {
  const SetupJobStartRequest({
    required this.correlationId,
    required this.jobId,
    required this.planRevision,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final SetupJobId jobId;
  final int planRevision;
  final RequestId requestId;

  factory SetupJobStartRequest.fromJson(Map<String, dynamic> json) {
    return SetupJobStartRequest(
      correlationId: _contractString(json['correlation_id'], 'SetupJobStartRequest.correlation_id'),
      jobId: _contractString(json['job_id'], 'SetupJobStartRequest.job_id'),
      planRevision: _contractInt(json['plan_revision'], 'SetupJobStartRequest.plan_revision'),
      requestId: _contractString(json['request_id'], 'SetupJobStartRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'job_id': jobId,
      'plan_revision': planRevision,
      'request_id': requestId,
    };
  }
}

class SetupJobStartResponse {
  const SetupJobStartResponse({
    required this.correlationId,
    required this.job,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final SetupJobSnapshot job;
  final RequestId requestId;

  factory SetupJobStartResponse.fromJson(Map<String, dynamic> json) {
    return SetupJobStartResponse(
      correlationId: _contractString(json['correlation_id'], 'SetupJobStartResponse.correlation_id'),
      job: SetupJobSnapshot.fromJson(_contractMap(json['job'], 'SetupJobStartResponse.job')),
      requestId: _contractString(json['request_id'], 'SetupJobStartResponse.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'job': job.toJson(),
      'request_id': requestId,
    };
  }
}

enum SetupJobState {
  draftPlan('draft_plan'),
  awaitingApproval('awaiting_approval'),
  approved('approved'),
  active('active'),
  attentionRequired('attention_required'),
  ready('ready'),
  failed('failed'),
  cancelled('cancelled'),
  unknown('unknown'),
  ;

  const SetupJobState(this.wireValue);

  final String wireValue;

  static SetupJobState fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class SetupJobStatusRequest {
  const SetupJobStatusRequest({
    required this.correlationId,
    required this.jobId,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final SetupJobId jobId;
  final RequestId requestId;

  factory SetupJobStatusRequest.fromJson(Map<String, dynamic> json) {
    return SetupJobStatusRequest(
      correlationId: _contractString(json['correlation_id'], 'SetupJobStatusRequest.correlation_id'),
      jobId: _contractString(json['job_id'], 'SetupJobStatusRequest.job_id'),
      requestId: _contractString(json['request_id'], 'SetupJobStatusRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'job_id': jobId,
      'request_id': requestId,
    };
  }
}

class SetupJobStatusResponse {
  const SetupJobStatusResponse({
    required this.correlationId,
    required this.job,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final SetupJobSnapshot job;
  final RequestId requestId;

  factory SetupJobStatusResponse.fromJson(Map<String, dynamic> json) {
    return SetupJobStatusResponse(
      correlationId: _contractString(json['correlation_id'], 'SetupJobStatusResponse.correlation_id'),
      job: SetupJobSnapshot.fromJson(_contractMap(json['job'], 'SetupJobStatusResponse.job')),
      requestId: _contractString(json['request_id'], 'SetupJobStatusResponse.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'job': job.toJson(),
      'request_id': requestId,
    };
  }
}

enum SetupJobTerminalState {
  ready('ready'),
  attentionRequired('attention_required'),
  failed('failed'),
  cancelled('cancelled'),
  unknown('unknown'),
  ;

  const SetupJobTerminalState(this.wireValue);

  final String wireValue;

  static SetupJobTerminalState fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class SetupPlan {
  const SetupPlan({
    required this.components,
    required this.jobId,
    required this.model,
    required this.reasons,
    required this.requiredEffects,
    required this.resources,
    required this.revision,
    required this.runtimeDisplayName,
    required this.runtimeVersion,
    required this.schemaVersion,
    required this.warnings,
  });

  final List<SetupPlanComponent> components;
  final SetupJobId jobId;
  final ModelMetadata model;
  final List<SetupReason> reasons;
  final List<SetupEffectKind> requiredEffects;
  final ResourceEstimate resources;
  final int revision;
  final String runtimeDisplayName;
  final String? runtimeVersion;
  final int schemaVersion;
  final List<SetupWarning> warnings;

  factory SetupPlan.fromJson(Map<String, dynamic> json) {
    return SetupPlan(
      components: _contractList(json['components'], 'SetupPlan.components').map((item) => SetupPlanComponent.fromJson(_contractMap(item, 'SetupPlan.components[]'))).toList(growable: false),
      jobId: _contractString(json['job_id'], 'SetupPlan.job_id'),
      model: ModelMetadata.fromJson(_contractMap(json['model'], 'SetupPlan.model')),
      reasons: _contractList(json['reasons'], 'SetupPlan.reasons').map((item) => SetupReason.fromJson(_contractMap(item, 'SetupPlan.reasons[]'))).toList(growable: false),
      requiredEffects: _contractList(json['required_effects'], 'SetupPlan.required_effects').map((item) => SetupEffectKind.fromJson(item)).toList(growable: false),
      resources: ResourceEstimate.fromJson(_contractMap(json['resources'], 'SetupPlan.resources')),
      revision: _contractInt(json['revision'], 'SetupPlan.revision'),
      runtimeDisplayName: _contractString(json['runtime_display_name'], 'SetupPlan.runtime_display_name'),
      runtimeVersion: json['runtime_version'] == null ? null : _contractString(json['runtime_version'], 'SetupPlan.runtime_version'),
      schemaVersion: _contractInt(json['schema_version'], 'SetupPlan.schema_version'),
      warnings: _contractList(json['warnings'], 'SetupPlan.warnings').map((item) => SetupWarning.fromJson(_contractMap(item, 'SetupPlan.warnings[]'))).toList(growable: false),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'components': components.map((item) => item.toJson()).toList(growable: false),
      'job_id': jobId,
      'model': model.toJson(),
      'reasons': reasons.map((item) => item.toJson()).toList(growable: false),
      'required_effects': requiredEffects.map((item) => item.toJson()).toList(growable: false),
      'resources': resources.toJson(),
      'revision': revision,
      'runtime_display_name': runtimeDisplayName,
      'runtime_version': runtimeVersion,
      'schema_version': schemaVersion,
      'warnings': warnings.map((item) => item.toJson()).toList(growable: false),
    };
  }
}

class SetupPlanComponent {
  const SetupPlanComponent({
    required this.detail,
    required this.kind,
    required this.required,
    required this.title,
  });

  final String detail;
  final SetupPlanComponentKind kind;
  final bool required;
  final String title;

  factory SetupPlanComponent.fromJson(Map<String, dynamic> json) {
    return SetupPlanComponent(
      detail: _contractString(json['detail'], 'SetupPlanComponent.detail'),
      kind: SetupPlanComponentKind.fromJson(json['kind']),
      required: _contractBool(json['required'], 'SetupPlanComponent.required'),
      title: _contractString(json['title'], 'SetupPlanComponent.title'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'detail': detail,
      'kind': kind.toJson(),
      'required': required,
      'title': title,
    };
  }
}

enum SetupPlanComponentKind {
  runtime('runtime'),
  model('model'),
  storage('storage'),
  verification('verification'),
  unknown('unknown'),
  ;

  const SetupPlanComponentKind(this.wireValue);

  final String wireValue;

  static SetupPlanComponentKind fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class SetupPlanRequest {
  const SetupPlanRequest({
    required this.correlationId,
    required this.destination,
    required this.providerId,
    required this.recommendation,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final SetupDestinationCategory destination;
  final RuntimeProviderId providerId;
  final RecommendationPlan recommendation;
  final RequestId requestId;

  factory SetupPlanRequest.fromJson(Map<String, dynamic> json) {
    return SetupPlanRequest(
      correlationId: _contractString(json['correlation_id'], 'SetupPlanRequest.correlation_id'),
      destination: SetupDestinationCategory.fromJson(json['destination']),
      providerId: _contractString(json['provider_id'], 'SetupPlanRequest.provider_id'),
      recommendation: RecommendationPlan.fromJson(_contractMap(json['recommendation'], 'SetupPlanRequest.recommendation')),
      requestId: _contractString(json['request_id'], 'SetupPlanRequest.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'destination': destination.toJson(),
      'provider_id': providerId,
      'recommendation': recommendation.toJson(),
      'request_id': requestId,
    };
  }
}

class SetupPlanResponse {
  const SetupPlanResponse({
    required this.correlationId,
    required this.job,
    required this.requestId,
  });

  final CorrelationId correlationId;
  final SetupJobSnapshot job;
  final RequestId requestId;

  factory SetupPlanResponse.fromJson(Map<String, dynamic> json) {
    return SetupPlanResponse(
      correlationId: _contractString(json['correlation_id'], 'SetupPlanResponse.correlation_id'),
      job: SetupJobSnapshot.fromJson(_contractMap(json['job'], 'SetupPlanResponse.job')),
      requestId: _contractString(json['request_id'], 'SetupPlanResponse.request_id'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'correlation_id': correlationId,
      'job': job.toJson(),
      'request_id': requestId,
    };
  }
}

class SetupReason {
  const SetupReason({
    required this.code,
    required this.message,
  });

  final SetupReasonCode code;
  final String message;

  factory SetupReason.fromJson(Map<String, dynamic> json) {
    return SetupReason(
      code: SetupReasonCode.fromJson(json['code']),
      message: _contractString(json['message'], 'SetupReason.message'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'code': code.toJson(),
      'message': message,
    };
  }
}

enum SetupReasonCode {
  recommendationSelected('recommendation_selected'),
  existingModelReusable('existing_model_reusable'),
  acquisitionRequired('acquisition_required'),
  approvalRequired('approval_required'),
  storageVerified('storage_verified'),
  runtimeVerified('runtime_verified'),
  registrationVerified('registration_verified'),
  readinessVerified('readiness_verified'),
  unknown('unknown'),
  ;

  const SetupReasonCode(this.wireValue);

  final String wireValue;

  static SetupReasonCode fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

enum SetupRecoveryAction {
  reviewApproval('review_approval'),
  retry('retry'),
  restoreDestination('restore_destination'),
  freeStorage('free_storage'),
  restoreRuntime('restore_runtime'),
  checkPrerequisites('check_prerequisites'),
  contactSupport('contact_support'),
  noAction('no_action'),
  unknown('unknown'),
  ;

  const SetupRecoveryAction(this.wireValue);

  final String wireValue;

  static SetupRecoveryAction fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

enum SetupStage {
  draftPlan('draft_plan'),
  awaitingApproval('awaiting_approval'),
  approved('approved'),
  preparing('preparing'),
  checkingStorage('checking_storage'),
  acquiring('acquiring'),
  registering('registering'),
  verifyingRuntime('verifying_runtime'),
  verifyingModel('verifying_model'),
  runningTestInference('running_test_inference'),
  ready('ready'),
  attentionRequired('attention_required'),
  failed('failed'),
  cancelled('cancelled'),
  unknown('unknown'),
  ;

  const SetupStage(this.wireValue);

  final String wireValue;

  static SetupStage fromJson(Object? value) {
    for (final candidate in values) {
      if (candidate.wireValue == value) {
        return candidate;
      }
    }
    return unknown;
  }

  String toJson() => wireValue;
}

class SetupWarning {
  const SetupWarning({
    required this.code,
    required this.message,
  });

  final SetupWarningCode code;
  final String message;

  factory SetupWarning.fromJson(Map<String, dynamic> json) {
    return SetupWarning(
      code: SetupWarningCode.fromJson(json['code']),
      message: _contractString(json['message'], 'SetupWarning.message'),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'code': code.toJson(),
      'message': message,
    };
  }
}

enum SetupWarningCode {
  externalRuntimeModified('external_runtime_modified'),
  providerManagedStorage('provider_managed_storage'),
  integrityMetadataUnavailable('integrity_metadata_unavailable'),
  cancellationMayRetainEffects('cancellation_may_retain_effects'),
  destinationEvidenceIncomplete('destination_evidence_incomplete'),
  unknown('unknown'),
  ;

  const SetupWarningCode(this.wireValue);

  final String wireValue;

  static SetupWarningCode fromJson(Object? value) {
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
  runtimeStatus('runtime_status'),
  runtimeConsent('runtime_consent'),
  runtimeLifecycle('runtime_lifecycle'),
  runtimeModelInventory('runtime_model_inventory'),
  setupWorkflow('setup_workflow'),
  localChat('local_chat'),
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
