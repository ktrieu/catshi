CREATE TABLE markets (
    id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    description TEXT NOT NULL,
    state TEXT NOT NULL,
    owner_id INTEGER REFERENCES users(id) NOT NULL,
    message_id TEXT,
    channel_id TEXT,
    thread_id TEXT,
    details_msg_id TEXT,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    resolved_at TIMESTAMPTZ
);

CREATE INDEX markets_message_id_idx ON markets(message_id);