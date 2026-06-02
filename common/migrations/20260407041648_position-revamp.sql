DROP TABLE positions;

CREATE TABLE positions (
    id INTEGER PRIMARY KEY NOT NULL,
    quantity INTEGER NOT NULL,
    cost_basis INTEGER NOT NULL,
    instrument_id INTEGER REFERENCES instruments(id) NOT NULL,
    owner_id INTEGER REFERENCES users(id) NOT NULL
);

CREATE UNIQUE INDEX positions_instrument_id_owner_id_unique ON positions(instrument_id, owner_id);