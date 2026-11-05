# Persistence with SQLite

Enable `sqlite` on `spire` to store scraped records in SQLite. The integration
lives in `spire-sqlx` behind its own `sqlite` feature, independently of other
SQL drivers. SQL dependencies are kept out of `spire-core`. The SQLite driver
builds a bundled SQLite library, requiring a C compiler.

SQLite runs inside your process, so there is no database server to start.

## Opening a store

`SqliteStore::open(path)` creates the database file if needed and applies
Spire's schema. `SqliteStore::builder()` opens one with options:

| Setting | Default | Builder option |
|---|---|---|
| Journal mode | Write-ahead logging (WAL) | None |
| `synchronous` | `NORMAL` | `sync_always()` for `FULL` |
| Busy timeout | 5 seconds | `busy_timeout(duration)` |
| Connections | 1 | `max_connections(limit)` |

```rust,no_run
use std::time::Duration;

use spire::store::SqliteStore;

# async fn open() -> spire::Result<()> {
let store = SqliteStore::builder()
    .sync_always()
    .busy_timeout(Duration::from_secs(10))
    .open("crawl.sqlite")
    .await?;
# Ok(())
# }
```

By default, commits survive an application crash. A power loss or system crash
may roll back the latest commits, to an earlier consistent state, so a crawl
repeats some work and resumes. `sync_always()` keeps every commit through those
too, at a cost in throughput.

SQLite saves WAL mode in the file, so opening an existing database converts it,
and it keeps `-wal` and `-shm` files beside it. To share a database with an
application that manages its settings, [use your own pool](#using-your-own-pool).

One connection serializes SQLite work without lock contention, and suits most
crawls. SQLite allows one writer at a time, so more connections only help
concurrent reads.

`store.close().await` closes the store's connections, for every clone of it.

## Using your own pool

`SqliteStore::new(pool)` uses a SQLx pool you created, from the upstream crate
at `spire::store::sqlx`. It stores a handle, performs no SQL and changes no pool
options: journal mode, `synchronous`, busy timeout and connection limits remain
yours, so durability follows your configuration. Call
`store.initialize().await?` before using storage services. In-memory databases
need their own pool. They disappear when their connections close.

```rust,no_run
use spire::store::SqliteStore;
use spire::store::sqlx::sqlite::SqlitePoolOptions;

# async fn open() -> Result<(), spire::BoxError> {
let pool = SqlitePoolOptions::new()
    .max_connections(1)
    .connect("sqlite:crawl.sqlite?mode=rwc")
    .await?;
let store = SqliteStore::new(pool);
store.initialize().await?;
# Ok(())
# }
```

With more than one connection, configure a busy timeout and consider WAL.

## Schema migrations

`open` and `initialize` apply embedded, versioned SQL migrations to the
`spire_dataset_*`, `spire_seen`, `spire_page_versions`, `spire_frontier_*` and
`spire_frontier_scheduled*` tables and indexes. Spire records applied versions
and checksums in `_spire_sqlx_migrations`, leaving the application's
`_sqlx_migrations` table and other application schema untouched.

Migrations use one write transaction, including migration history. Concurrent
runs serialize; repeated runs are safe. If a migration fails, its schema changes
and history are rolled back together. Untracked tables with reserved Spire names
are rejected rather than adopted. Changed migration checksums and versions
unknown to this library also produce an error. The connection must permit schema
creation and upgrades.

New library migrations run only when you open or initialize a store. Do so
before opening a frontier, and stop existing crawls that use the database
before applying upgrades. Keep released migration files unchanged and add
new versions for later schema changes. Your JSON record schema and application
migrations remain your responsibility.

## A crawl with persistent results

This example uses offline pages; enable `sqlite` and `testing`. The same
dataset works with a real HTTP or browser backend.

```rust,no_run
use spire::extract::{Data, html::Html};
use spire::frontier::FrontierExt;
use spire::frontier::seed::Seed;
use spire::store::SqliteStore;
use spire::testing::{MockPage, MockSite};
use spire::worker::{WorkerExt, middleware::Revalidate};
use spire::{Crawler, Result, Router};

async fn page(html: Html, titles: Data<String>) -> Result<()> {
    titles.write(html.title().unwrap_or_default()).await
}

# async fn crawl() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
let store = SqliteStore::open("crawl.sqlite").await?;
let site = MockSite::new().page("https://example.com/", MockPage::html("<title>Example Domain</title>"));
let worker = Router::new().fallback(page)
    .layer(Revalidate::new(store.validator()));
let frontier = store.frontier().await?.dedup_with(store.seen()).retry(3);
let crawler = Crawler::new(site, worker)
    .with_frontier(frontier)
    .with_dataset(store.dataset::<String>("titles"))
    .with_seed(Seed::get("https://example.com/")?);
crawler.run().await?;
for title in crawler.dataset::<String>()?.read_all().await? {
    println!("{title}");
}
drop(crawler); // Release frontier ownership before another crawl opens it.
store.close().await;
# Ok(())
# }
```

The facade exports the store's services beside their traits: `SqliteDataset`
in `spire::dataset`, `SqliteFrontier` in `spire::frontier`, `SqliteSeen` in
`spire::frontier::seen` and `SqliteValidator` in `spire::worker::middleware`.

`SqliteStoreBuilder` is in `spire::store`, beside the store.

Direct integrations can use `spire_sqlx::sqlite::SqliteStore` and the
`spire_sqlx::sqlx` crate re-export instead of the facade.

## Ordering, batches and failures

Datasets are named collections within one shared table, not separate tables
or application row mappings. Names are bound SQL values, so punctuation does
not become executable SQL. Each name must consistently use the same serde
record type. JSON schema changes are the application's responsibility.

Writes append records. Reads **take and remove** the oldest records; they
are not snapshots or repeated queries. IDs assigned while holding SQLite's
writer lock define FIFO. A batch preserves its input order; concurrent
writers are ordered by lock acquisition, not task start time. Concurrent
consumers, including independent pools or processes, cannot successfully
take the same item twice. Their futures may finish in a different order.

Batch writes serialize all records first and insert them in one transaction.
Batch reads select, decode and delete in one `BEGIN IMMEDIATE` transaction,
avoiding competing read-to-write lock upgrades. Any SQL or decode failure
rolls the batch back. A malformed record can block reads until repaired or
removed through `store.pool()`; inspect and repair before retrying.
SQLite failures are reported as `ErrorKind::Dataset`, with the original
error preserved as the source. `size_hint()` returns `None`; use the async
`len()` when an exact current count is needed.

Reads commit before returning. Cancellation or a crash after commit can lose
delivery to the caller: this is a destructive dataset, not an acknowledged
work queue. Dataset writes and frontier updates are separate transactions,
even if both are persistent. A page replayed after a crash can write records
again. Deduplicate using an application key when consuming results.

## Seen sets and validators

`store.seen()` implements `SeenSet`, atomically recording each fingerprint
once across concurrent connections. Use it with `.dedup_with(store.seen())`
on a frontier to retain deduplication across restarts. Fingerprints are
stored as eight-byte blobs so the entire unsigned 64-bit range is preserved.

`store.validator()` implements `Validator` for the `Revalidate` middleware.
ETag and Last-Modified are stored as raw bytes, including non-UTF-8 values.
Replacing a page version also clears headers absent from its replacement.
Both services share the store's pool and durability settings. Their `len()`
and `is_empty()` queries are asynchronous.

For cached model answers sharing this file, see
[Model budgets and caches](agent-resources.md#sqlite-answer-caches).

## Frontier ownership and recovery

`store.frontier().await?` opens one persistent priority queue per database
file. Higher priorities run first, with insertion order breaking ties. Due
retries and scheduled revisits join the queue and compete by priority, rather
than always preceding fresh requests. `.revisit::<YourTags>()` supplies the usual
revisit behavior. Scheduled times are persisted at millisecond resolution;
while open, elapsed time follows the monotonic clock. The frontier keeps the
latest time it saw, so a wall clock moved back between runs does not delay due
times, while one moved forward brings them forward.

Opening the frontier acquires an OS lock on
`<database>.spire-frontier.lock`, then requeues requests abandoned in flight.
Calls through clones of one store share this ownership. A separate store or
process is refused while any frontier handle is alive. Dropping the last
handle releases the lock; the OS also releases it on process exit. No
heartbeat task or expiry policy is involved. The store retains only a weak
reference, so keeping the pool or store alive does not retain the lock.

Leave the sidecar file in place. Deleting it while a frontier is active can
allow two owners to lock different files under the same name. Use a local
filesystem with advisory-lock support, and do not access the database through
hard-link aliases. In-memory databases support datasets and other services,
but cannot host a persistent frontier. Other pools can still use datasets,
seen sets, validators and caches while one frontier owns the file.

Enqueueing or scheduling a seed serializes it. Caller-attached `Seed` extensions
are discarded at that boundary, even without a restart, as with redb. Register
runtime services with `Crawler::with_extension` so they are supplied to every
request, including recovered requests. Keep per-request data that must survive
persistence in serializable seed fields, such as its tag.

Popping a seed atomically marks it in flight. Reporting completion, failure
or a skip removes it; cancellation requeues it. Each queued occurrence is
stored separately, so duplicate requests do not overwrite in-flight records.
A corrupt seed returns an error and remains stored for repair. A seed waiting
for a retry is stored with its attempt and due time, so a restart keeps both,
as with redb.

Frontier pops and reports are separate transactions from dataset writes and
seen-set admission. Delivery is at least once across recovery; a page can run
again after its output committed. Adding a seed through a deduplication
facade and enqueueing it are not one atomic transaction. For recovery, do not
rely on re-admitting abandoned seeds through a seen set: the frontier restores
its own in-flight rows directly.
