CREATE TABLE markets (
    id INTEGER PRIMARY KEY NOT NULL,
    description TEXT NOT NULL, 
    state TEXT NOT NULL,
    owner_id INTEGER REFERENCES users(id) NOT NULL,
    message_id TEXT
);