# Crash safety with redb

With the `redb` feature, a `RedbStore` keeps a whole crawl in one file on
disk: its queue, the URLs it has seen, what it scraped, the validators of the
pages it fetched, and its scheduled revisits. Reopening the file resumes the
crawl where it stopped, after a clean stop or a crash.

```rust,no_run
use serde::{Deserialize, Serialize};
use spire::backend::HttpClient;
use spire::frontier::FrontierExt;
use spire::frontier::seed::Seed;
use spire::redb::RedbStore;
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
    .layer(Revalidate::new(store.validators()));
let crawler = Crawler::new(HttpClient::default(), router)
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
| `frontier()` | Queued seeds, by priority then insertion order, and scheduled revisits | [The frontier](frontier.md), [Recrawls](recrawls.md) |
| `seen()` | Every fingerprint seen, exact, on disk | [The frontier](frontier.md) |
| `dataset::<T>(name)` | Items of type `T`, a table per name | [Storing results](datasets.md) |
| `validators()` | Pages' `ETag` and `Last-Modified` | [Recrawls](recrawls.md) |

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

The store's file is the crawl's own. redb locks its file and lets one
transaction write at a time, so an application's own tables, e.g. a search
index, belong in a database of their own, where they do not wait on the
frontier's every commit. `spire::redb::redb` re-exports redb at the version
spire uses.
