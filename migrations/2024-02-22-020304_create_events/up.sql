CREATE TABLE events (
    id TEXT PRIMARY KEY NOT NULL,
    url TEXT NOT NULL,
    referrer TEXT,
    name TEXT NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    collector_id TEXT NOT NULL
);