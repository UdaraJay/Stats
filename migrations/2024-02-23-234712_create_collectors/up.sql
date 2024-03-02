CREATE TABLE collectors (
    id TEXT PRIMARY KEY NOT NULL,
    origin TEXT NOT NULL,
    country TEXT NOT NULL,
    city TEXT NOT NULL,
    os TEXT,
    browser TEXT,
    timestamp TIMESTAMP NOT NULL
);