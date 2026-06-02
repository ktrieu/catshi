CREATE TABLE instruments (
    id INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL, 
    state TEXT NOT NULL,
    market_id INTEGER REFERENCES markets(id) NOT NULL
);