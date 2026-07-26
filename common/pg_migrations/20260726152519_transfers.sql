CREATE TABLE transfers (
    id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    amount INTEGER NOT NULL,
    sender INTEGER REFERENCES users(id) NOT NULL,
    receiver INTEGER REFERENCES users(id) NOT NULL,
    memo TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    source TEXT NOT NULL
);

CREATE INDEX transfers_sender_idx ON transfers(sender);
CREATE INDEX transfers_receiver_idx ON transfers(receiver);