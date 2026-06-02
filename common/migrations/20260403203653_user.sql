CREATE TABLE users (
    id INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    discord_id TEXT NOT NULL
);

CREATE UNIQUE INDEX users_discord_id_unique ON users(discord_id);