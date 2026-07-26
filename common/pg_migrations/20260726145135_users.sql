CREATE TABLE users (
    id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name TEXT NOT NULL,
    discord_id TEXT NOT NULL,
    cash_balance BIGINT NOT NULL
);

CREATE UNIQUE INDEX users_discord_id_unique ON users(discord_id);