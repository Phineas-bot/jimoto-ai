import 'package:gixgiz_desktop/core/generated/core_contracts.g.dart';

RecommendationPlan setupRecommendationFixture() {
  return RecommendationPlan.fromJson({
    'catalogue_version': 'gixgiz-catalogue-v0.1.0',
    'rule_set_version': 'gixgiz-capability-rules-v0.1.0',
    'role': 'recommended',
    'compatibility': 'compatible',
    'model': {
      'catalogue_id': 'gixgiz.model.fixture',
      'display_name': 'Fixture model',
      'family': 'Fixture family',
      'licence_spdx': 'Apache-2.0',
      'provenance_url': 'https://example.invalid/models/fixture',
      'size_class': 'standard',
      'workload_tiers': ['general_text'],
    },
    'runtime': {
      'catalogue_id': 'gixgiz.runtime.local-text',
      'display_name': 'Local model runtime',
      'supports_cpu_only': true,
      'supported_architectures': ['x86_64'],
      'optional_accelerations': <Object?>[],
    },
    'resources': setupResourcesFixture,
    'confidence': 'high',
    'reasons': [
      {'code': 'workload_match', 'message': 'Matches the selected use.'},
    ],
    'warnings': <Object?>[],
  });
}

SetupJobSnapshot setupJobFixture({
  String state = 'awaiting_approval',
  String stage = 'awaiting_approval',
  Map<String, Object?>? progress,
  Map<String, Object?>? effectReport,
  List<Map<String, Object?>>? reasons,
  List<Map<String, Object?>>? warnings,
  SetupRecoveryAction? recoveryAction,
  bool cancellationRequested = false,
  int latestEventSequence = 0,
  int updatedAtUnixMs = 4,
}) {
  return SetupJobSnapshot.fromJson({
    'schema_version': 1,
    'job_id': setupJobIdFixture,
    'plan': {
      'schema_version': 1,
      'job_id': setupJobIdFixture,
      'revision': 1,
      'model': {
        'schema_version': 1,
        'artifact': {
          'canonical_model_id': 'gixgiz.model.fixture',
          'provider_id': setupProviderIdFixture,
          'provider_model_id': 'fixture:latest',
          'source_summary': 'Official provider model mapping.',
        },
        'display_name': 'Fixture model',
        'family': 'Fixture family',
        'size_class': 'standard',
        'licence_spdx': 'Apache-2.0',
        'provenance': 'https://example.invalid/models/fixture',
        'destination': 'provider_managed',
        'destination_display': 'Provider-managed local storage',
        'expected_size_bytes': 2147483648,
        'measured_size_bytes': state == 'ready' ? 2147483648 : null,
        'catalogue_version': 'gixgiz-catalogue-v0.1.0',
        'rule_set_version': 'gixgiz-capability-rules-v0.1.0',
        'lifecycle_state': switch (state) {
          'active' => 'acquiring',
          'ready' => 'available',
          'attention_required' => 'attention_required',
          'failed' => 'failed',
          'cancelled' => 'cancelled',
          _ => 'planned',
        },
        'verification_state': state == 'ready' ? 'verified' : 'not_started',
        'verification': state == 'ready'
            ? {
                'runtime_health_verified': true,
                'model_available': true,
                'registration_verified': true,
                'inference_verified': true,
                'integrity': 'provider_reported',
                'verified_at_unix_ms': updatedAtUnixMs,
              }
            : null,
      },
      'runtime_display_name': 'Local model runtime',
      'runtime_version': '0.11.10',
      'resources': setupResourcesFixture,
      'components': [
        {
          'kind': 'model',
          'title': 'Fixture model',
          'detail': 'Add the exact selected provider artifact.',
          'required': true,
        },
      ],
      'required_effects': [
        'provider_model_acquisition',
        'provider_model_registration',
        'readiness_inference',
        'metadata_persistence',
      ],
      'reasons':
          reasons ??
          const [
            {
              'code': 'recommendation_selected',
              'message': 'Selected by the deterministic capability report.',
            },
          ],
      'warnings':
          warnings ??
          const [
            {
              'code': 'provider_managed_storage',
              'message': 'The provider owns this storage location.',
            },
          ],
    },
    'state': state,
    'stage': stage,
    'progress': progress,
    'attention_reason': state == 'attention_required'
        ? 'recovery_required'
        : null,
    'recovery_action':
        recoveryAction?.toJson() ??
        switch (state) {
          'attention_required' || 'failed' || 'cancelled' => 'retry',
          _ => null,
        },
    'error': null,
    'effect_report': effectReport,
    'cancellation_report': null,
    'cancellation_requested': cancellationRequested,
    'retry_count': 0,
    'latest_event_sequence': latestEventSequence,
    'created_at_unix_ms': 1,
    'updated_at_unix_ms': updatedAtUnixMs,
  });
}

SetupJobEvent setupEventFixture({
  required int sequence,
  required SetupJobSnapshot job,
  String kind = 'progress',
  String? terminalState,
}) {
  return SetupJobEvent.fromJson({
    'schema_version': 1,
    'job_id': setupJobIdFixture,
    'correlation_id': setupCorrelationIdFixture,
    'sequence': sequence,
    'kind': kind,
    'timestamp_unix_ms': job.updatedAtUnixMs,
    'job': job.toJson(),
    'terminal_state': terminalState,
  });
}

const setupJobIdFixture = '00000000-0000-4000-8000-000000000010';
const setupCorrelationIdFixture = '00000000-0000-4000-8000-000000000011';
const setupProviderIdFixture = 'gixgiz.runtime.test.v1';

const setupResourcesFixture = <String, Object?>{
  'memory': {
    'required_bytes': 4294967296,
    'safety_margin_bytes': 2147483648,
    'observed_total_bytes': 17179869184,
    'observed_available_bytes': 8589934592,
  },
  'storage': {
    'required_bytes': 2147483648,
    'safety_margin_bytes': 2147483648,
    'observed_free_bytes': 32212254720,
  },
  'planned_context_tokens': 8192,
  'cpu_only': true,
  'gpu_memory_bytes': null,
  'acceleration': null,
};
