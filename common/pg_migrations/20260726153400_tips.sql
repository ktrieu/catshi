CREATE TABLE tips (
    id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    channel_id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    amount INTEGER NOT NULL,
    user_id INTEGER NOT NULL REFERENCES users(id),
    transfer_id INTEGER NOT NULL REFERENCES transfers(id)
);

CREATE UNIQUE INDEX tips_channel_id_message_id_user_id_unique_idx ON tips(channel_id, message_id, user_id);