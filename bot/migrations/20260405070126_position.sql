CREATE TABLE positions (
    id INTEGER PRIMARY KEY NOT NULL,
    state TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    enter_price INTEGER NOT NULL,
    -- Null if not exited, of course.
    exit_price INTEGER,
    instrument_id INTEGER REFERENCES instruments(id) NOT NULL,
    owner_id INTEGER REFERENCES users(id) NOT NULL
);

CREATE INDEX positions_instrument_id ON positions(instrument_id);
CREATE INDEX positions_owner_id ON positions(owner_id);