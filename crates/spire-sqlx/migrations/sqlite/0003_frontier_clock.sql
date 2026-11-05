-- Frontier clock schema. Spire owns this table; application migrations are
-- tracked separately from this migration.

-- The latest time the frontier saw, as UTC milliseconds since the Unix epoch,
-- so a wall clock moved back between runs does not delay due times.
CREATE TABLE spire_frontier_clock (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    last_seen INTEGER NOT NULL
);
