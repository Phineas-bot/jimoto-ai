-- Durable state for approved managed runtime installation.
--
-- Rust and SQLite own every transition. The table deliberately stores only
-- categorical metadata and bounded text: never the installer binary, raw
-- process output, credentials, full command lines, or private paths.
CREATE TABLE runtime_install_jobs (
    job_id TEXT PRIMARY KEY CHECK (length(job_id) BETWEEN 1 AND 64),
    provider_id TEXT NOT NULL CHECK (length(provider_id) BETWEEN 1 AND 128),
    plan_revision INTEGER NOT NULL CHECK (plan_revision >= 1),
    version TEXT NOT NULL CHECK (length(version) BETWEEN 1 AND 64),
    source_origin TEXT NOT NULL CHECK (length(source_origin) BETWEEN 1 AND 512),
    artifact_name TEXT NOT NULL CHECK (length(artifact_name) BETWEEN 1 AND 256),
    expected_size_bytes INTEGER NOT NULL CHECK (expected_size_bytes >= 0),
    requires_administrator INTEGER NOT NULL CHECK (requires_administrator IN (0, 1)),
    state TEXT NOT NULL CHECK (
        state IN (
            'awaiting_approval', 'approved', 'running',
            'attention_required', 'ready', 'failed', 'cancelled', 'unknown'
        )
    ),
    stage TEXT NOT NULL CHECK (length(stage) BETWEEN 1 AND 64),
    approved_revision INTEGER CHECK (approved_revision IS NULL OR approved_revision >= 1),
    approved_at_unix_ms INTEGER CHECK (approved_at_unix_ms IS NULL OR approved_at_unix_ms >= 0),
    transferred_bytes INTEGER CHECK (transferred_bytes IS NULL OR transferred_bytes >= 0),
    verification_json TEXT CHECK (verification_json IS NULL OR length(verification_json) <= 512),
    effects_json TEXT NOT NULL CHECK (length(effects_json) <= 4096),
    attention TEXT CHECK (attention IS NULL OR length(attention) BETWEEN 1 AND 64),
    failure TEXT CHECK (failure IS NULL OR length(failure) BETWEEN 1 AND 64),
    recovery TEXT CHECK (recovery IS NULL OR length(recovery) BETWEEN 1 AND 64),
    ownership TEXT NOT NULL CHECK (
        ownership IN ('external', 'gix_giz_managed', 'bundled', 'unknown')
    ),
    retry_count INTEGER NOT NULL CHECK (retry_count >= 0),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= 0)
);

-- At most one installation job may be non-terminal at a time, so an approved
-- system change can never be started twice concurrently.
CREATE UNIQUE INDEX runtime_install_jobs_single_active
    ON runtime_install_jobs (provider_id)
    WHERE state NOT IN ('ready', 'failed', 'cancelled');

CREATE INDEX runtime_install_jobs_updated
    ON runtime_install_jobs (updated_at_unix_ms DESC, job_id DESC);
