CREATE TABLE instruments (
    id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name TEXT NOT NULL,
    state TEXT NOT NULL,
    market_id INTEGER REFERENCES markets(id) NOT NULL
);

CREATE INDEX instruments_market_id_idx ON instruments(market_id);