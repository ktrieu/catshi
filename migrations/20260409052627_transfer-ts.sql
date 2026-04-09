DROP TABLE transfers;

CREATE TABLE transfers (
    id INTEGER PRIMARY KEY NOT NULL,
    amount INTEGER NOT NULL,
    sender INTEGER REFERENCES users(id) NOT NULL,
    receiver INTEGER REFERENCES users(id) NOT NULL,
    memo TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);