# Persistence with redb

With the `redb` feature, a `RedbStore` keeps a whole crawl in one file on
disk: its queue, the URLs it has seen, what it scraped, the versions of the
pages it fetched, and its scheduled revisits. Reopening the file resumes the
crawl where it stopped, after a clean stop or a crash.

```rust,no_run
use serde::{Deserialize, Serialize};
use spire::backend::ReqwestBackend;
use spire::frontier::FrontierExt;
use spire::frontier::seed::Seed;
use spire::store::RedbStore;
use spire::worker::WorkerExt;
use spire::worker::middleware::Revalidate;
use spire::{Crawler, Result, Router};

#[derive(Serialize, Deserialize)]
struct Product {
    name: String,
}
# async fn page() {}

# async fn crawl() -> Result<()> {
let store = RedbStore::open("crawl.redb").await?;
let router = Router::new()
    .fallback(page)
    .layer(Revalidate::new(store.validator()));
let crawler = Crawler::new(ReqwestBackend::default(), router)
    .with_frontier(store.frontier().dedup_with(store.seen()).retry(3))
    .with_dataset(store.dataset::<Product>("products"))
    // Refused as already seen when the crawl resumes.
    .with_seed(Seed::get("https://example.com/")?);
crawler.run().await?;
# Ok(())
# }
```

| From the store | Holds | See |
|---|---|---|
| `frontier()` | Queued seeds, by priority then insertion order, retries waiting to run, and scheduled revisits | [Seeds and the frontier](frontier.md), [Recrawls](recrawls.md) |
| `seen()` | Every fingerprint seen, exact, on disk | [Seeds and the frontier](frontier.md) |
| `dataset::<T>(name)` | Items of type `T`, a table per name | [Storing results](datasets.md) |
| `validator()` | Pages' `ETag` and `Last-Modified` | [Recrawls](recrawls.md) |

The facade exports these services beside their traits: `RedbFrontier` in
`spire::frontier`, `RedbSeen` in `spire::frontier::seen`, `RedbDataset` in
`spire::dataset` and `RedbValidator` in `spire::worker::middleware`.

## What survives a crash

Every change is a committed transaction, and nothing is lost:

- **Requests in flight are crawled again.** A popped seed stays recorded
  until its outcome is reported, so the requests running at a crash are
  queued again on reopening: a request may run twice, never zero times.
- **No link is lost.** A page's links are committed before the page is
  reported done, so a crash either keeps them, or keeps the page in flight,
  to be crawled again and find them again.
- **Items may be written twice.** Items and the frontier are committed
  separately, so a page whose items were committed just before a crash may be
  crawled again and write them again: deduplicate by a key such as the URL
  when reading.
- **Retries keep their place.** A seed waiting for a retry is stored with its
  attempt and when it is due, so it neither starts its retries over nor runs
  early. Due times follow a monotonic clock while the store is open, and the
  wall clock across a restart. The store keeps the latest time it saw, so a
  clock moved back between runs does not delay them, while one moved forward
  brings them forward.
- **Revisits are kept, one per page.** A revisit is scheduled before its
  page is reported, so a crash in between crawls the page once more rather
  than losing its revisit.

## Syncing

Commits reach the disk at most once a second. A crash rolls the store back
to its last synced commit, a consistent earlier state, so it repeats at most
a second of work. `RedbStore::builder().sync_always()` syncs every commit
instead, repeating only the requests in flight, at a large cost in
throughput: in the `redb` benchmark on macOS, about 0.6 ms per seed batched
against 11 ms synced each time.

## Your own tables

Tables that serve the crawl can share its file. Clones of a store share one
database, and `read` and `write` run your own transactions on it, so small
crawl-related state can be saved and reopened with the crawl. The agent's
answer cache works this way: `AgentStoreExt::agent_cache()` keeps its answers
in a namespaced table of the crawl's store, see
[Model budgets and caches](agent-resources.md).

```rust,no_run
use spire::store::RedbStore;
use spire::store::redb::{TableDefinition, TableError};

// Namespaced and versioned, so it stays clear of the store's own tables.
const VISITS: TableDefinition<&str, u64> = TableDefinition::new("my-app/visits/v1");

# async fn visits(store: RedbStore) -> spire::Result<()> {
// Runs off the async runtime, and commits when the callback returns `Ok`.
store
    .write(|txn| {
        txn.open_table(VISITS)?.insert("example.com", 1)?;
        Ok(())
    })
    .await?;

// Returning an error rolls back what the callback wrote, and hands the error
// back. `?` works on redb's errors and spire's, and `into()` converts your own.
let rejected: spire::Result<()> = store
    .write(|txn| {
        txn.open_table(VISITS)?.insert("example.com", 2)?;
        Err(spire::Error::invalid("too many visits").into())
    })
    .await;
assert!(rejected.is_err());

// Runs on the calling thread, so async code moves it to the blocking pool.
let reader = store.clone();
let visits = tokio::task::spawn_blocking(move || {
    reader.read(|txn| match txn.open_table(VISITS) {
        Ok(table) => Ok(table.get("example.com")?.map(|visits| visits.value())),
        // Tables are created by their first write.
        Err(TableError::TableDoesNotExist(_)) => Ok(None),
        Err(error) => Err(error.into()),
    })
})
.await
.expect("the read should not panic")?;
assert_eq!(visits, Some(1));
# Ok(())
# }
```

Leave the tables spire uses alone: every name starting with `spire/` or
`spire-agent/`. Leave durability alone too: `write` applies the store's [syncing](#syncing) policy to your commits.

Your tables get the same crash behaviour as the crawl's, with two consequences:

- **An `Ok` is not yet on disk.** Under a sync interval, a crash rolls back
  commits since the last sync. The crawl repeats its own lost work, but nothing
  repeats a write of yours made outside it. Use `sync_always` if that matters.
- **Writes from a handler may repeat.** Your commit is separate from the
  frontier's, so a page crawled again after a crash runs its handler again: a
  counter may count that page twice. Prefer writes that are safe to repeat, such
  as storing a value by URL.

redb locks its file and lets one transaction write at a time, across every
table in it, so your writes wait on the frontier's commits, and it on yours.
An unrelated or write-heavy workload, e.g. an application's search index,
belongs in a database of its own. `spire::store::redb` re-exports redb at the
version spire uses.
