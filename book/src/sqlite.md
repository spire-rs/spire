# Persistence with SQLite

Enable `sqlite` on `spire` to store scraped records in SQLite. The integration
lives in `spire-sqlx` behind its own `sqlite` feature, independently of other
SQL drivers. SQL dependencies are kept out of `spire-core`. The SQLite driver
builds a bundled SQLite library, requiring a C compiler.

SQLite runs inside your process. Opening a pool opens the database file;
`create_if_missing(true)` creates it when needed. There is no database server
to start.

## Pool and schema ownership

The application creates and owns the SQLx pool. `SqliteStore::new(pool)` only
stores a pool handle; it performs no SQL and changes no pool options.
Call `store.initialize().await?` explicitly before using storage services.
It applies embedded, versioned SQL migrations to the `spire_dataset_*`,
`spire_seen`, `spire_validators`, `spire_frontier_*` and `spire_scheduled*`
tables and indexes. Spire records applied versions and checksums in
`_spire_sqlx_migrations`, leaving the application's `_sqlx_migrations` table
and other application schema untouched.

Initialization uses one write transaction, including migration history.
Concurrent initializers serialize; repeated calls are safe. If a migration
fails, its schema changes and history are rolled back together. Untracked
tables with reserved Spire names are rejected rather than adopted. Changed
migration checksums and versions unknown to this library also produce an
error. The pool must permit schema creation and upgrades.

New library migrations run only when `initialize()` is called. Initialize
before opening a frontier, and stop existing crawls that use the database
before applying upgrades. Keep released migration files unchanged and add
new versions for later schema changes.
Your JSON record schema and application migrations remain your responsibility.

For a simple crawl, one connection serializes SQLite work without lock
contention. For more connections, configure a busy timeout and consider WAL.
SQLite still allows only one writer at a time. The store never changes
journal mode, `synchronous`, busy timeout, or connection limits. Durability
therefore follows the caller's SQLite configuration; use `synchronous=FULL`
when stronger commit durability is required. In-memory databases disappear
when their connections close; a file-backed database can be reopened.

## A crawl with persistent results

This example uses offline pages; enable `sqlite` and `testing`. The same
dataset works with a real HTTP or browser backend.

```rust,no_run
use std::time::Duration;

use spire::extract::{Data, html::Html};
use spire::frontier::FrontierExt;
use spire::frontier::seed::Seed;
use spire::sqlite::SqliteStore;
use spire::sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteSynchronous};
use spire::testing::{MockPage, MockSite};
use spire::worker::{WorkerExt, middleware::Revalidate};
use spire::{Crawler, Result, Router};

async fn page(html: Html, titles: Data<String>) -> Result<()> {
    titles.write(html.title().unwrap_or_default()).await
}

# async fn crawl() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
let pool = SqlitePoolOptions::new()
    .max_connections(1)
    .connect_with(
        SqliteConnectOptions::new()
            .filename("crawl.sqlite")
            .create_if_missing(true)
            .busy_timeout(Duration::from_secs(5))
            .synchronous(SqliteSynchronous::Full),
    )
    .await?;
let store = SqliteStore::new(pool.clone());
store.initialize().await?;
let site = MockSite::new().page("https://example.com/", MockPage::html("<title>Example Domain</title>"));
let worker = Router::new().fallback(page)
    .layer(Revalidate::new(store.validators()));
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
pool.close().await;
# Ok(())
# }
```

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
removed through the caller's pool; inspect and repair before retrying.
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

`store.validators()` implements `Validators` for the `Revalidate` middleware.
ETag and Last-Modified are stored as raw bytes, including non-UTF-8 values.
Replacing a validator also clears headers absent from its replacement.
Both services share the store's pool and durability settings. Their `len()`
and `is_empty()` queries are asynchronous.

For cached model answers sharing this file, see
[Model budgets and caches](agent-resources.md#sqlite-answer-caches).

## Frontier ownership and recovery

`store.frontier().await?` opens one persistent priority queue per database
file. Higher priorities run first, with insertion order breaking ties. Due
scheduled revisits join the queue and compete by priority, rather than
always preceding fresh requests. `.revisit::<YourTags>()` supplies the usual
revisit behavior. Scheduled times are persisted at millisecond resolution;
while open, elapsed time follows the monotonic clock.

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

Popping a seed atomically marks it in flight. Reporting completion, failure
or a skip removes it; cancellation requeues it. Each queued occurrence is
stored separately, so duplicate requests do not overwrite in-flight records.
A corrupt seed returns an error and remains stored for repair. Retry facades
hold delayed retries in memory, while their original records stay in flight;
a restart recovers them at their stored attempt, as with redb. The separate
retry facade's ordering limitations still apply.

Frontier pops and reports are separate transactions from dataset writes and
seen-set admission. Delivery is at least once across recovery; a page can run
again after its output committed. Adding a seed through a deduplication
facade and enqueueing it are not one atomic transaction. For recovery, do not
rely on re-admitting abandoned seeds through a seen set: the frontier restores
its own in-flight rows directly.
