-- Backing table for the api's tower-sessions SessionStore. The session id is
-- tower-sessions' base64-encoded Id, and data is its session record's data
-- map as plain JSON, so common doesn't depend on tower-sessions.
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    data JSONB NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX sessions_expires_at_idx ON sessions(expires_at);
