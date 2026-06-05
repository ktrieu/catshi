CREATE TABLE cf_games (
    id INTEGER PRIMARY KEY NOT NULL,
    published BOOLEAN NOT NULL DEFAULT false
);

CREATE TABLE cf_articles (
    id INTEGER PRIMARY KEY NOT NULL,
    -- These are actually arrays but SQLite says we can't have them.
    -- Separated by | character in the DB.
    names TEXT NOT NULL,
    categories TEXT NOT NULL,
    article_order INTEGER NOT NULL,
    game_id INTEGER REFERENCES cf_games(id) NOT NULL
);

CREATE UNIQUE INDEX cf_articles_game_id_order_idx ON cf_articles(game_id, article_order);