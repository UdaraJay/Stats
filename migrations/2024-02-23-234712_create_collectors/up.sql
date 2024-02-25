CREATE TABLE collectors (
    id TEXT PRIMARY KEY,
    origin TEXT NOT NULL,
    country TEXT NOT NULL,
    city TEXT NOT NULL,
    timestamp TIMESTAMP NOT NULL
);