-- Initial crawl storage schema. Spire owns these tables and indexes;
-- application migrations are tracked separately from this migration.

-- Named JSON datasets share one table. The insertion sequence preserves FIFO
-- within each dataset, and records are deleted when successfully consumed.
CREATE TABLE spire_dataset_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    dataset TEXT NOT NULL,
    payload TEXT NOT NULL
);

CREATE INDEX spire_dataset_order
    ON spire_dataset_items (dataset, id);

-- Exact request fingerprints are opaque eight-byte values, preserving the
-- full unsigned 64-bit range without SQLite's signed integer limitation.
CREATE TABLE spire_seen (
    fingerprint BLOB PRIMARY KEY NOT NULL CHECK(length(fingerprint) = 8)
);

-- Conditional HTTP request headers retain their raw bytes, including non-UTF-8
-- values. A missing header is NULL rather than an empty header value.
CREATE TABLE spire_validators (
    fingerprint BLOB PRIMARY KEY NOT NULL CHECK(length(fingerprint) = 8),
    etag BLOB,
    last_modified BLOB
);

-- Each queued occurrence has its own ID, even when fingerprints repeat.
-- Popped requests remain in flight until acknowledged; the next exclusive
-- frontier owner requeues abandoned in-flight rows after a process exits.
CREATE TABLE spire_frontier_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    fingerprint BLOB NOT NULL,
    priority INTEGER NOT NULL,
    payload TEXT NOT NULL,
    in_flight INTEGER NOT NULL DEFAULT 0 CHECK(in_flight IN (0, 1))
);

-- Ready requests run by descending priority, then insertion order.
CREATE INDEX spire_frontier_order
    ON spire_frontier_items (in_flight, priority DESC, id);

-- Each fingerprint has at most one scheduled revisit. Due times are persisted
-- as UTC milliseconds since the Unix epoch and survive process restarts.
CREATE TABLE spire_frontier_scheduled (
    fingerprint BLOB PRIMARY KEY NOT NULL,
    due INTEGER NOT NULL,
    priority INTEGER NOT NULL,
    payload TEXT NOT NULL
);

CREATE INDEX spire_frontier_scheduled_due
    ON spire_frontier_scheduled (due);
