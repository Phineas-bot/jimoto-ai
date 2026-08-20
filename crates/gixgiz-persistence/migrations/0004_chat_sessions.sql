CREATE TABLE conversations (
    conversation_id TEXT PRIMARY KEY CHECK (length(conversation_id) = 36),
    title TEXT NOT NULL CHECK (length(CAST(title AS BLOB)) BETWEEN 1 AND 120),
    canonical_model_id TEXT REFERENCES models(canonical_model_id),
    runtime_provider_id TEXT NOT NULL CHECK (
        length(runtime_provider_id) BETWEEN 1 AND 128
    ),
    next_sequence INTEGER NOT NULL DEFAULT 1 CHECK (next_sequence > 0),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= created_at_unix_ms)
);

CREATE INDEX conversations_recent_idx
    ON conversations (updated_at_unix_ms DESC, conversation_id);

CREATE TABLE messages (
    message_id TEXT PRIMARY KEY CHECK (length(message_id) = 36),
    conversation_id TEXT NOT NULL
        REFERENCES conversations(conversation_id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
    status TEXT NOT NULL CHECK (
        status IN ('pending', 'generating', 'completed', 'cancelled', 'failed')
    ),
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    content TEXT NOT NULL CHECK (length(CAST(content AS BLOB)) <= 65536),
    generation_id TEXT CHECK (generation_id IS NULL OR length(generation_id) = 36),
    cancellation_requested INTEGER NOT NULL DEFAULT 0 CHECK (
        cancellation_requested IN (0, 1)
    ),
    last_event_sequence INTEGER NOT NULL DEFAULT 0 CHECK (last_event_sequence >= 0),
    failure_code TEXT CHECK (
        failure_code IS NULL OR length(failure_code) BETWEEN 1 AND 128
    ),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms >= 0),
    updated_at_unix_ms INTEGER NOT NULL CHECK (updated_at_unix_ms >= created_at_unix_ms),
    completed_at_unix_ms INTEGER CHECK (
        completed_at_unix_ms IS NULL OR completed_at_unix_ms >= 0
    ),
    UNIQUE (conversation_id, sequence),
    CHECK (
        role <> 'user'
        OR (status = 'completed' AND generation_id IS NULL AND failure_code IS NULL)
    ),
    CHECK (role <> 'assistant' OR generation_id IS NOT NULL),
    CHECK (
        status NOT IN ('completed', 'cancelled', 'failed')
        OR completed_at_unix_ms IS NOT NULL
    ),
    CHECK (
        status IN ('completed', 'cancelled', 'failed')
        OR completed_at_unix_ms IS NULL
    ),
    CHECK (failure_code IS NULL OR status = 'failed')
);

CREATE UNIQUE INDEX messages_one_active_generation_per_conversation
    ON messages (conversation_id)
    WHERE status = 'generating';

CREATE UNIQUE INDEX messages_generation_idx
    ON messages (generation_id)
    WHERE generation_id IS NOT NULL;

CREATE INDEX messages_ordered_idx ON messages (conversation_id, sequence);
