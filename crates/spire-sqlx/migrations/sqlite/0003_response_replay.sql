-- Frontier response replay schema. Spire owns this table;
-- application migrations are tracked separately from this migration.

-- Snapshots belong to individual frontier occurrences, not URLs or identities.
-- They survive interrupted handlers and are removed with the frontier report.
CREATE TABLE spire_frontier_responses (
    frontier_id INTEGER PRIMARY KEY NOT NULL,
    head TEXT NOT NULL,
    body BLOB NOT NULL
);
