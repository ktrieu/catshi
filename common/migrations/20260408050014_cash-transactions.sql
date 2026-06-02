CREATE TABLE transfers (
    id INTEGER PRIMARY KEY NOT NULL,
    amount INTEGER NOT NULL,
    sender INTEGER REFERENCES users(id) NOT NULL,
    receiver INTEGER REFERENCES users(id) NOT NULL,
    memo TEXT NOT NULL
);

ALTER TABLE users ADD COLUMN cash_balance INTEGER NOT NULL DEFAULT 0;

-- Insert a "system user" that's the counterparty for all orders.
INSERT INTO users (
    name,
    discord_id,
    cash_balance
) VALUES (
    'Central Bank',
    '0',
    -- 10,000,000 YP
    10000000000
);