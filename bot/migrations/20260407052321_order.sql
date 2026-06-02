CREATE TABLE orders (
    id INTEGER PRIMARY KEY NOT NULL,
    direction TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    shares_price INTEGER NOT NULL,
    fees INTEGER NOT NULL,
    cost_basis INTEGER NOT NULL,
    instrument_id INTEGER REFERENCES instruments(id) NOT NULL,
    owner_id INTEGER REFERENCES users(id) NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX orders_owner_id ON orders(owner_id);