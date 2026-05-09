CREATE TABLE tips (
    id INTEGER PRIMARY KEY NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    channel_id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    amount INTEGER NOT NULL,
    user_id INTEGER NOT NULL REFERENCES users(id),
    transfer_id INTEGER NOT NULL REFERENCES transfers(id)
);

CREATE UNIQUE INDEX tips_channel_id_message_id_user_id_unique_idx ON tips(channel_id, message_id, user_id);