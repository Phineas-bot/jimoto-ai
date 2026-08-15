CREATE TABLE models (
    canonical_model_id TEXT PRIMARY KEY CHECK (
        length(canonical_model_id) BETWEEN 1 AND 128
    ),
    display_name TEXT NOT NULL CHECK (length(CAST(display_name AS BLOB)) BETWEEN 1 AND 256),
    family TEXT NOT NULL CHECK (length(CAST(family AS BLOB)) BETWEEN 1 AND 128),
    size_class TEXT NOT NULL CHECK (
        size_class IN ('compact', 'standard', 'large')
    ),
    licence_spdx TEXT NOT NULL CHECK (length(CAST(licence_spdx AS BLOB)) BETWEEN 1 AND 128),
    provenance_url TEXT NOT NULL CHECK (length(CAST(provenance_url AS BLOB)) BETWEEN 1 AND 2048),
    catalogue_version TEXT NOT NULL CHECK (
        length(CAST(catalogue_version AS BLOB)) BETWEEN 1 AND 128
    ),
    expected_size_bytes INTEGER NOT NULL CHECK (expected_size_bytes >= 0),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= created_at_unix_ms)
);

CREATE TABLE setup_jobs (
    job_id TEXT PRIMARY KEY REFERENCES durable_jobs(id) ON DELETE CASCADE,
    plan_schema_version INTEGER NOT NULL CHECK (plan_schema_version > 0),
    plan_revision INTEGER NOT NULL CHECK (plan_revision > 0),
    state TEXT NOT NULL CHECK (
        state IN (
            'draft_plan',
            'awaiting_approval',
            'approved',
            'preparing',
            'checking_storage',
            'acquiring',
            'registering',
            'verifying_runtime',
            'verifying_model',
            'running_test_inference',
            'ready',
            'attention_required',
            'failed',
            'cancelled'
        )
    ),
    stage TEXT NOT NULL CHECK (
        stage IN (
            'plan_review',
            'approval',
            'preparation',
            'storage',
            'acquisition',
            'registration',
            'runtime_verification',
            'model_verification',
            'inference_verification',
            'complete',
            'attention',
            'failed',
            'cancelled'
        )
    ),
    canonical_model_id TEXT NOT NULL REFERENCES models(canonical_model_id),
    model_display_name TEXT NOT NULL CHECK (
        length(CAST(model_display_name AS BLOB)) BETWEEN 1 AND 256
    ),
    model_family TEXT NOT NULL CHECK (
        length(CAST(model_family AS BLOB)) BETWEEN 1 AND 128
    ),
    model_size_class TEXT NOT NULL CHECK (
        model_size_class IN ('compact', 'standard', 'large')
    ),
    licence_spdx TEXT NOT NULL CHECK (
        length(CAST(licence_spdx AS BLOB)) BETWEEN 1 AND 128
    ),
    provenance_url TEXT NOT NULL CHECK (
        length(CAST(provenance_url AS BLOB)) BETWEEN 1 AND 2048
    ),
    provider_id TEXT NOT NULL CHECK (length(provider_id) BETWEEN 1 AND 128),
    provider_model_id TEXT NOT NULL CHECK (length(provider_model_id) BETWEEN 1 AND 256),
    artifact_source_summary TEXT NOT NULL CHECK (
        length(CAST(artifact_source_summary AS BLOB)) BETWEEN 1 AND 256
    ),
    destination_display TEXT NOT NULL CHECK (
        length(CAST(destination_display AS BLOB)) BETWEEN 1 AND 256
    ),
    runtime_display_name TEXT NOT NULL CHECK (
        length(CAST(runtime_display_name AS BLOB)) BETWEEN 1 AND 256
    ),
    runtime_version TEXT CHECK (runtime_version IS NULL OR length(runtime_version) BETWEEN 1 AND 128),
    catalogue_version TEXT NOT NULL CHECK (length(catalogue_version) BETWEEN 1 AND 128),
    rule_set_version TEXT NOT NULL CHECK (length(rule_set_version) BETWEEN 1 AND 128),
    destination_category TEXT NOT NULL CHECK (
        destination_category IN ('provider_managed', 'application_data', 'user_selected')
    ),
    expected_size_bytes INTEGER NOT NULL CHECK (expected_size_bytes >= 0),
    expected_memory_bytes INTEGER NOT NULL CHECK (expected_memory_bytes >= 0),
    memory_safety_margin_bytes INTEGER NOT NULL CHECK (memory_safety_margin_bytes >= 0),
    storage_safety_margin_bytes INTEGER NOT NULL CHECK (storage_safety_margin_bytes >= 0),
    planned_context_tokens INTEGER NOT NULL CHECK (planned_context_tokens > 0),
    cpu_only INTEGER NOT NULL CHECK (cpu_only IN (0, 1)),
    gpu_memory_bytes INTEGER CHECK (gpu_memory_bytes IS NULL OR gpu_memory_bytes >= 0),
    observed_total_memory_bytes INTEGER CHECK (
        observed_total_memory_bytes IS NULL OR observed_total_memory_bytes >= 0
    ),
    observed_available_memory_bytes INTEGER CHECK (
        observed_available_memory_bytes IS NULL OR observed_available_memory_bytes >= 0
    ),
    observed_free_storage_bytes INTEGER CHECK (
        observed_free_storage_bytes IS NULL OR observed_free_storage_bytes >= 0
    ),
    acceleration TEXT CHECK (
        acceleration IS NULL OR acceleration IN ('direct_ml', 'cuda', 'rocm')
    ),
    external_runtime_effect INTEGER NOT NULL CHECK (external_runtime_effect IN (0, 1)),
    progress_phase TEXT CHECK (
        progress_phase IS NULL
        OR progress_phase IN ('preparing', 'transferring', 'verifying', 'registering', 'completed')
    ),
    progress_completed_bytes INTEGER CHECK (
        progress_completed_bytes IS NULL OR progress_completed_bytes >= 0
    ),
    progress_total_bytes INTEGER CHECK (
        progress_total_bytes IS NULL OR progress_total_bytes >= 0
    ),
    progress_basis_points INTEGER CHECK (
        progress_basis_points IS NULL OR progress_basis_points BETWEEN 0 AND 10000
    ),
    cancellation_requested INTEGER NOT NULL DEFAULT 0 CHECK (cancellation_requested IN (0, 1)),
    retry_count INTEGER NOT NULL DEFAULT 0 CHECK (retry_count >= 0),
    last_safe_error_code TEXT CHECK (
        last_safe_error_code IS NULL OR length(last_safe_error_code) BETWEEN 1 AND 128
    ),
    attention_reason TEXT CHECK (
        attention_reason IS NULL OR length(attention_reason) BETWEEN 1 AND 128
    ),
    runtime_verified_at_unix_ms INTEGER CHECK (
        runtime_verified_at_unix_ms IS NULL OR runtime_verified_at_unix_ms >= 0
    ),
    model_available_at_unix_ms INTEGER CHECK (
        model_available_at_unix_ms IS NULL OR model_available_at_unix_ms >= 0
    ),
    registration_verified_at_unix_ms INTEGER CHECK (
        registration_verified_at_unix_ms IS NULL OR registration_verified_at_unix_ms >= 0
    ),
    inference_verified_at_unix_ms INTEGER CHECK (
        inference_verified_at_unix_ms IS NULL OR inference_verified_at_unix_ms >= 0
    ),
    approved_at_unix_ms INTEGER CHECK (approved_at_unix_ms IS NULL OR approved_at_unix_ms >= 0),
    cancelled_at_unix_ms INTEGER CHECK (cancelled_at_unix_ms IS NULL OR cancelled_at_unix_ms >= 0),
    last_event_sequence INTEGER NOT NULL DEFAULT 0 CHECK (last_event_sequence >= 0),
    revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= created_at_unix_ms),
    CHECK (
        progress_completed_bytes IS NULL
        OR progress_total_bytes IS NULL
        OR progress_completed_bytes <= progress_total_bytes
    ),
    CHECK (
        state NOT IN (
            'approved',
            'preparing',
            'checking_storage',
            'acquiring',
            'registering',
            'verifying_runtime',
            'verifying_model',
            'running_test_inference',
            'ready'
        )
        OR approved_at_unix_ms IS NOT NULL
    ),
    CHECK (
        state <> 'ready'
        OR (
            runtime_verified_at_unix_ms IS NOT NULL
            AND model_available_at_unix_ms IS NOT NULL
            AND registration_verified_at_unix_ms IS NOT NULL
            AND inference_verified_at_unix_ms IS NOT NULL
        )
    )
);

CREATE UNIQUE INDEX setup_jobs_one_nonterminal_idx
    ON setup_jobs ((1))
    WHERE state NOT IN ('ready', 'failed', 'cancelled');

CREATE INDEX setup_jobs_recovery_idx
    ON setup_jobs (state, updated_at_unix_ms, job_id);

CREATE TABLE setup_plan_notices (
    job_id TEXT NOT NULL REFERENCES setup_jobs(job_id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    kind TEXT NOT NULL CHECK (kind IN ('reason', 'warning')),
    code TEXT NOT NULL CHECK (length(code) BETWEEN 1 AND 128),
    message TEXT NOT NULL CHECK (length(CAST(message AS BLOB)) BETWEEN 1 AND 512),
    PRIMARY KEY (job_id, ordinal)
);

CREATE TABLE setup_plan_effects (
    job_id TEXT NOT NULL REFERENCES setup_jobs(job_id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    effect_code TEXT NOT NULL CHECK (length(effect_code) BETWEEN 1 AND 128),
    PRIMARY KEY (job_id, ordinal),
    UNIQUE (job_id, effect_code)
);

CREATE TABLE setup_approvals (
    job_id TEXT PRIMARY KEY REFERENCES setup_jobs(job_id) ON DELETE CASCADE,
    plan_revision INTEGER NOT NULL CHECK (plan_revision > 0),
    decision TEXT NOT NULL CHECK (decision IN ('approve', 'deny')),
    canonical_model_id TEXT NOT NULL CHECK (length(canonical_model_id) BETWEEN 1 AND 128),
    provider_id TEXT NOT NULL CHECK (length(provider_id) BETWEEN 1 AND 128),
    provider_model_id TEXT NOT NULL CHECK (length(provider_model_id) BETWEEN 1 AND 256),
    destination_category TEXT NOT NULL CHECK (
        destination_category IN ('provider_managed', 'application_data', 'user_selected')
    ),
    expected_size_bytes INTEGER NOT NULL CHECK (expected_size_bytes >= 0),
    licence_spdx TEXT NOT NULL CHECK (length(CAST(licence_spdx AS BLOB)) BETWEEN 1 AND 128),
    provenance_url TEXT NOT NULL CHECK (length(CAST(provenance_url AS BLOB)) BETWEEN 1 AND 2048),
    external_runtime_effect INTEGER NOT NULL CHECK (external_runtime_effect IN (0, 1)),
    correlation_id TEXT NOT NULL CHECK (length(correlation_id) = 36),
    request_id TEXT NOT NULL CHECK (length(request_id) = 36),
    decided_at_unix_ms INTEGER NOT NULL CHECK (decided_at_unix_ms >= 0)
);

CREATE TABLE setup_approval_effects (
    job_id TEXT NOT NULL REFERENCES setup_approvals(job_id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    effect_code TEXT NOT NULL CHECK (length(effect_code) BETWEEN 1 AND 128),
    PRIMARY KEY (job_id, ordinal),
    UNIQUE (job_id, effect_code)
);

CREATE TABLE setup_job_events (
    job_id TEXT NOT NULL REFERENCES setup_jobs(job_id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    correlation_id TEXT NOT NULL CHECK (length(correlation_id) = 36),
    kind TEXT NOT NULL CHECK (
        kind IN (
            'snapshot',
            'stage_changed',
            'progress',
            'cancellation_requested',
            'retry',
            'recovery',
            'terminal'
        )
    ),
    state TEXT NOT NULL CHECK (length(state) BETWEEN 1 AND 64),
    stage TEXT NOT NULL CHECK (length(stage) BETWEEN 1 AND 64),
    progress_phase TEXT CHECK (
        progress_phase IS NULL
        OR progress_phase IN ('preparing', 'transferring', 'verifying', 'registering', 'completed')
    ),
    progress_completed_bytes INTEGER CHECK (
        progress_completed_bytes IS NULL OR progress_completed_bytes >= 0
    ),
    progress_total_bytes INTEGER CHECK (
        progress_total_bytes IS NULL OR progress_total_bytes >= 0
    ),
    progress_basis_points INTEGER CHECK (
        progress_basis_points IS NULL OR progress_basis_points BETWEEN 0 AND 10000
    ),
    safe_code TEXT CHECK (safe_code IS NULL OR length(safe_code) BETWEEN 1 AND 128),
    occurred_at_unix_ms INTEGER NOT NULL CHECK (occurred_at_unix_ms >= 0),
    PRIMARY KEY (job_id, sequence),
    CHECK (
        progress_completed_bytes IS NULL
        OR progress_total_bytes IS NULL
        OR progress_completed_bytes <= progress_total_bytes
    )
);

CREATE TABLE setup_effects (
    job_id TEXT NOT NULL REFERENCES setup_jobs(job_id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    disposition TEXT NOT NULL CHECK (
        disposition IN ('completed', 'retained', 'rolled_back', 'uncertain')
    ),
    effect_code TEXT NOT NULL CHECK (length(effect_code) BETWEEN 1 AND 128),
    size_bytes INTEGER CHECK (size_bytes IS NULL OR size_bytes >= 0),
    occurred_at_unix_ms INTEGER NOT NULL CHECK (occurred_at_unix_ms >= 0),
    PRIMARY KEY (job_id, ordinal)
);

CREATE TABLE model_provider_artifacts (
    canonical_model_id TEXT NOT NULL REFERENCES models(canonical_model_id),
    provider_id TEXT NOT NULL CHECK (length(provider_id) BETWEEN 1 AND 128),
    provider_model_id TEXT NOT NULL CHECK (length(provider_model_id) BETWEEN 1 AND 256),
    artifact_source_summary TEXT NOT NULL CHECK (
        length(CAST(artifact_source_summary AS BLOB)) BETWEEN 1 AND 256
    ),
    destination_display TEXT NOT NULL CHECK (
        length(CAST(destination_display AS BLOB)) BETWEEN 1 AND 256
    ),
    destination_category TEXT NOT NULL CHECK (
        destination_category IN ('provider_managed', 'application_data', 'user_selected')
    ),
    expected_size_bytes INTEGER NOT NULL CHECK (expected_size_bytes >= 0),
    measured_size_bytes INTEGER CHECK (measured_size_bytes IS NULL OR measured_size_bytes >= 0),
    lifecycle_state TEXT NOT NULL CHECK (
        lifecycle_state IN (
            'planned',
            'acquiring',
            'acquired_unverified',
            'verifying',
            'available',
            'retained_unverified',
            'rejected',
            'failed'
        )
    ),
    verification_state TEXT NOT NULL CHECK (
        verification_state IN ('not_started', 'pending', 'verified', 'failed', 'unknown')
    ),
    integrity_state TEXT NOT NULL CHECK (
        integrity_state IN ('unavailable', 'provider_reported', 'verified', 'mismatch', 'unknown')
    ),
    last_setup_job_id TEXT REFERENCES setup_jobs(job_id) ON DELETE SET NULL,
    registered_at_unix_ms INTEGER CHECK (
        registered_at_unix_ms IS NULL OR registered_at_unix_ms >= 0
    ),
    verified_at_unix_ms INTEGER CHECK (verified_at_unix_ms IS NULL OR verified_at_unix_ms >= 0),
    updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= 0),
    PRIMARY KEY (canonical_model_id, provider_id),
    UNIQUE (provider_id, provider_model_id),
    CHECK (
        lifecycle_state <> 'available'
        OR (
            verification_state = 'verified'
            AND integrity_state IN ('provider_reported', 'verified')
            AND registered_at_unix_ms IS NOT NULL
            AND verified_at_unix_ms IS NOT NULL
        )
    )
);
