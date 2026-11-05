-- Frontier retries schema. Spire owns this table and index; application
-- migrations are tracked separately from this migration.

-- Seeds waiting for a retry, each occurrence separately. Due times are
-- persisted as UTC milliseconds since the Unix epoch and survive restarts.
CREATE TABLE spire_frontier_retries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    fingerprint BLOB NOT NULL,
    due INTEGER NOT NULL,
    priority INTEGER NOT NULL,
    payload TEXT NOT NULL
);

CREATE INDEX spire_frontier_retries_due
    ON spire_frontier_retries (due, id);
