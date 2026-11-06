# Persistence with SQLite

With the `sqlite` feature, a `SqliteStore` keeps a whole crawl in one SQLite
database: its queue, the URLs it has seen, what it scraped, and the versions
of the pages it fetched. Reopening the database resumes the crawl where it
stopped, after a clean stop or a crash. SQLite runs inside your process, so
there is no server to start; its driver builds a bundled SQLite, which needs a
C compiler.

```rust,no_run
use spire::backend::ReqwestBackend;
use spire::extract::Data;
use spire::extract::html::Html;
use spire::frontier::FrontierExt;
use spire::frontier::seed::Seed;
use spire::store::SqliteStore;
use spire::worker::{WorkerExt, middleware::Revalidate};
use spire::{Crawler, Result, Router};

async fn page(html: Html, titles: Data<String>) -> Result<()> {
    titles.write(html.title().unwrap_or_default()).await
}

# async fn crawl() -> Result<()> {
let store = SqliteStore::open("crawl.sqlite").await?;
let router = Router::new()
    .fallback(page)
    .layer(Revalidate::new(store.validator()));
let crawler = Crawler::new(ReqwestBackend::default(), router)
    .with_frontier(store.frontier().await?.dedup_with(store.seen()).retry(3))
    .with_dataset(store.dataset::<String>("titles"))
    // Refused as already seen when the crawl resumes.
    .with_seed(Seed::get("https://example.com/")?);
crawler.run().await?;
drop(crawler); // Releases the frontier, so another crawl can open it.
store.close().await;
# Ok(())
# }
```

| From the store | Holds | See |
|---|---|---|
| `frontier().await?` | Queued seeds, by priority then insertion order, retries waiting to run, and scheduled revisits | [Seeds and the frontier](frontier.md), [Recrawls](recrawls.md) |
| `seen()` | Every fingerprint seen, exact | [Seeds and the frontier](frontier.md) |
| `dataset::<T>(name)` | Items of type `T`, by name | [Storing results](datasets.md) |
| `validator()` | Pages' `ETag` and `Last-Modified` | [Recrawls](recrawls.md) |

The facade exports these services beside their traits: `SqliteFrontier` in
`spire::frontier`, `SqliteSeen` in `spire::frontier::seen`, `SqliteDataset` in
`spire::dataset` and `SqliteValidator` in `spire::worker::middleware`. The
model answer cache can share the database too; see
[Model budgets and caches](agent-resources.md#sqlite-answer-caches).

## What survives a crash

Every change is a committed transaction, and nothing is lost:

- **Requests in flight are crawled again.** A popped seed stays recorded
  until its outcome is reported, so the requests running at a crash are
  queued again on reopening: a request may run twice, never zero times.
- **No link is lost.** A page's links are committed before the page is
  reported done, and a link is recorded as seen only once it is queued, so a
  crash in between queues it again rather than refusing it.
- **Items may be written twice.** Items and the frontier are committed
  separately, so a page whose items were committed just before a crash may be
  crawled again and write them again: deduplicate by a key such as the URL
  when reading.
- **Retries and revisits keep their place.** A seed waiting for a retry is
  stored with its attempt and when it is due, and revisits with when they are
  due. Due times follow a monotonic clock while the database is open, and the
  wall clock across a restart. The frontier records the latest time it saw
  when it pops or holds a seed, so after a clock is moved back between runs,
  a retry waits no longer than it had left then; a clock moved forward brings
  due times forward.

A seed is serialized when it is queued, so extensions attached to it at
runtime are dropped, even without a restart. Register shared services with
`Crawler::with_extension` instead, and keep per-request data in the seed's
own fields, such as its tag.

## Syncing

`SqliteStore::open` uses write-ahead logging (WAL) and `synchronous=NORMAL`:
every commit survives the process crashing, and a power loss or system crash
may roll back the latest commits to a consistent earlier state, so the crawl
repeats some work. `SqliteStore::builder()` changes the defaults:

| Setting | Default | Builder option |
|---|---|---|
| Journal mode | WAL | |
| `synchronous` | `NORMAL` | `sync_always()`, for `FULL`: every commit survives a power loss, at a cost in throughput |
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

SQLite allows one writer at a time, so one connection suits most crawls, and
more help only concurrent reads. WAL mode is saved in the file, so opening an
existing database converts it, and SQLite keeps `-wal` and `-shm` files beside
it. `store.close().await` closes the store's connections, for every clone.

## Your own pool

`SqliteStore::new(pool)` uses a SQLx pool you created, with the crate
re-exported at `spire::store::sqlx`, to share a database with an application
that manages its own settings. The store changes none of the pool's options,
so durability follows your configuration; with more than one connection, set
a busy timeout. Call `initialize` before using the store:

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

An in-memory database needs a pool of your own, and disappears when its
connections close. It hosts datasets, seen sets and validators, but not a
frontier, which needs a file.

## Datasets

Each named dataset stores its items as JSON, in one shared table; give each
name one item type. Reads **take and remove** the oldest items, in the order
they were written: they are not snapshots. Concurrent readers, even in other
processes, never take the same item twice.

A batch is one transaction: a batch write inserts all its items or none, and
a batch read that cannot decode an item takes none, leaving the batch stored.
Such an item blocks reads until it is repaired or removed through
`store.pool()`. A read commits before it returns, so a crash or cancellation
right after it can lose those items to the caller.

## Schema migrations

`open` and `initialize` apply Spire's schema, as embedded migrations to its
own `spire_*` tables, recording them in `_spire_sqlx_migrations`. The
application's `_sqlx_migrations` table and other tables are left alone, so an
application keeps its own migrations beside them. Migrating is one
transaction, safe to repeat or run concurrently; a failed migration rolls back
completely. A changed or unknown migration, or an untracked table with one of
Spire's names, is rejected instead of being adopted.

A newer Spire migrates the database when a store opens it. Stop the crawls
using the database before upgrading, and open the store before opening a
frontier.

## Frontier ownership

A database has one frontier owner at a time. `store.frontier().await?` takes
an OS lock on `<database>.spire-frontier.lock`, then queues again the requests
a crashed run left in flight. Clones of one store share the frontier, while
another store or process is refused until the last handle is dropped, or the
process exits. Datasets, seen sets and validators stay shared meanwhile.

Leave the lock file in place: deleting it while a frontier is open lets two
owners lock different files under one name. Keep the database on a local
filesystem with advisory locks, and do not reach it through hard links.
