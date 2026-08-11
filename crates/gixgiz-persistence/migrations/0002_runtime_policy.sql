CREATE TABLE runtime_policy (
    provider_id TEXT PRIMARY KEY CHECK (length(provider_id) BETWEEN 1 AND 128),
    ownership TEXT NOT NULL CHECK (
        ownership IN ('external', 'gix_giz_managed', 'bundled', 'unknown')
    ),
    reuse_consent TEXT NOT NULL CHECK (
        reuse_consent IN ('not_requested', 'reuse_approved', 'denied', 'unknown')
    ),
    management_consent TEXT NOT NULL CHECK (
        management_consent IN ('not_requested', 'management_approved', 'denied', 'unknown')
    ),
    updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= 0)
);
