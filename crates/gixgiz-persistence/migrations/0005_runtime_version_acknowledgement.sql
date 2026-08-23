-- Records the exact provider version a user accepted while it remains outside
-- recorded support evidence. The acknowledgement is deliberately bound to one
-- exact version: a later provider version is untested again and needs a new
-- explicit decision. NULL means no acknowledgement exists.
ALTER TABLE runtime_policy
    ADD COLUMN acknowledged_untested_version TEXT
    CHECK (
        acknowledged_untested_version IS NULL
        OR length(acknowledged_untested_version) BETWEEN 1 AND 64
    );
