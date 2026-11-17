CREATE TABLE cf_archive (
    id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    game_number INT NOT NULL,
    message_id TEXT NOT NULL,
    results JSONB NOT NULL,
    posted_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX cf_archive_game_number_idx ON cf_archive(game_number);
CREATE UNIQUE INDEX cf_archive_message_id_unique_idx ON cf_archive(message_id);

CREATE TABLE cf_archive_users (
    archive_id INT NOT NULL REFERENCES cf_archive(id) ON DELETE CASCADE,
    user_id INT NOT NULL REFERENCES users(id),
    PRIMARY KEY (archive_id, user_id)
);

CREATE INDEX cf_archive_users_user_id_idx ON cf_archive_users(user_id);
