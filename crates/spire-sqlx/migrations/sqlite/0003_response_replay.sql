-- Frontier response replay schema. Spire owns this table;
-- application migrations are tracked separately from this migration.

-- Snapshots belong to individual frontier occurrences, not URLs or identities.
-- They survive interrupted handlers and are removed with the frontier report.
CREATE TABLE spire_frontier_responses (
    frontier_id INTEGER PRIMARY KEY NOT NULL,
    head TEXT NOT NULL,
    body BLOB NOT NULL
);

-- Rotate the token when recovering a frontier. Queued writes from an older
-- owner must check it inside their statement before changing snapshots.
CREATE TABLE spire_frontier_session (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    token BLOB NOT NULL
);

INSERT INTO spire_frontier_session (id, token) VALUES (1, randomblob(16));
