CREATE TABLE blackjacks (
    id INTEGER PRIMARY KEY NOT NULL,
    dealer TEXT NOT NULL,
    player TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'open',
    channel_id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    owner_id INTEGER NOT NULL REFERENCES users(id),
    staked INTEGER NOT NULL
);

CREATE UNIQUE INDEX blackjack_channel_id_message_id_uniq ON blackjacks(channel_id, message_id);