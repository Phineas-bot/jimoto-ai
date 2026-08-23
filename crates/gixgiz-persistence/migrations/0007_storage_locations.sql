-- User-selected storage locations for runtimes, models, and downloads.
--
-- One row per kind. Absence of a row means the built-in default applies, so a
-- user who never chooses a location has no stored path at all.
--
-- The path is the one place GixGiz deliberately stores a filesystem location:
-- the user chose it and must be shown it. It is bounded and validated by Rust
-- before it is written.
CREATE TABLE storage_locations (
    kind TEXT PRIMARY KEY CHECK (kind IN ('runtime', 'models', 'staging')),
    category TEXT NOT NULL CHECK (
        category IN ('provider_managed', 'application_data', 'user_selected', 'unknown')
    ),
    path TEXT CHECK (path IS NULL OR length(path) BETWEEN 1 AND 240),
    -- Records that the user explicitly accepted reconfiguring a runtime GixGiz
    -- does not own, so the transition is auditable rather than implicit.
    external_consent_at_unix_ms INTEGER
        CHECK (external_consent_at_unix_ms IS NULL OR external_consent_at_unix_ms >= 0),
    updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= 0),
    -- A user-selected location must carry a path; a default must not.
    CHECK (
        (category = 'user_selected' AND path IS NOT NULL)
        OR (category <> 'user_selected' AND path IS NULL)
    )
);
