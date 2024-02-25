CREATE TABLE events (
    id TEXT PRIMARY KEY,
    url TEXT NOT NULL,
    name TEXT NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    collector_id TEXT NOT NULL
);