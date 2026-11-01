-- Validated model answers share the crawl's SQLite database. The agent owns
-- this table and tracks its schema independently of crawl storage migrations.
-- Agent-generated keys include content, instructions, schema, token limit,
-- and the caller's provider/model namespace; they are not page URLs.
CREATE TABLE spire_agent_cache (
    key TEXT PRIMARY KEY NOT NULL,
    answer TEXT NOT NULL
);
