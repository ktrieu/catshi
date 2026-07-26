CREATE TABLE blackjacks (
    id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    dealer TEXT NOT NULL,
    player TEXT NOT NULL,
    state TEXT NOT NULL,
    channel_id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    owner_id INTEGER NOT NULL REFERENCES users(id),
    staked INTEGER NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX blackjack_channel_id_message_id_unique ON blackjacks(channel_id, message_id);