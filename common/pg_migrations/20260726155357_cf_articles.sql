CREATE TABLE cf_articles (
    id INTEGER GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    names TEXT[] NOT NULL,
    categories TEXT[] NOT NULL,
    article_order INT NOT NULL,
    game_id INTEGER REFERENCES cf_games(id) NOT NULL
);

CREATE UNIQUE INDEX cf_articles_game_id_order_idx ON cf_articles(game_id, article_order);