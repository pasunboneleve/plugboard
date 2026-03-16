CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    topic TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at TEXT NOT NULL,
    parent_id TEXT REFERENCES messages(id),
    conversation_id TEXT NOT NULL,
    producer TEXT,
    metadata_json TEXT
);

CREATE INDEX IF NOT EXISTS idx_messages_topic_created_at
    ON messages(topic, created_at);

CREATE INDEX IF NOT EXISTS idx_messages_conversation_id
    ON messages(conversation_id);

CREATE TABLE IF NOT EXISTS claims (
    id TEXT PRIMARY KEY,
    message_id TEXT NOT NULL REFERENCES messages(id),
    runner_name TEXT NOT NULL,
    claimed_at TEXT NOT NULL,
    lease_until TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'completed', 'failed', 'timed_out')),
    completed_at TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_claims_active_message
    ON claims(message_id)
    WHERE status = 'active';

CREATE INDEX IF NOT EXISTS idx_claims_runner_status
    ON claims(runner_name, status);
