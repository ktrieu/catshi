CREATE TABLE cf_games (
    id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    published BOOLEAN NOT NULL DEFAULT false,
    owner_id INT NOT NULL REFERENCES users(id)
);

CREATE INDEX cf_games_owner_id_idx ON cf_games(owner_id);