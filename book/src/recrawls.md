# Recrawls

A crawl usually runs once: every page is fetched, and the crawl ends when
nothing is left. Keeping a dataset fresh takes two more things: crawling
pages again on a schedule, and skipping the ones that did not change.

## Revisiting pages

`FrontierExt::revisit` crawls pages again after an interval that depends on
their tag. The intervals belong to the tag enum, through the `Revisit`
trait, which `#[derive(Revisit)]` implements from a `#[revisit("...")]`
attribute on each variant to revisit:

```rust,no_run
use std::time::Duration;

use spire::backend::HttpClient;
use spire::frontier::seed::Seed;
use spire::frontier::{FrontierExt, InMemFrontier};
use spire::request::{Revisit, Tags};
use spire::{Crawler, Result, Router};

#[derive(Debug, Clone, Copy, Tags, Revisit)]
enum Page {
    #[revisit("1h")]
    Listing,
    #[revisit("1d")]
    Product,
    // No attribute: crawled once.
    About,
}
# async fn listing() {}
# async fn product() {}
# async fn about() {}

# async fn crawl() -> Result<()> {
let router = Router::new()
    .route(Page::Listing, listing)
    .route(Page::Product, product)
    .route(Page::About, about);
let crawler = Crawler::new(HttpClient::default(), router)
    .with_frontier(InMemFrontier::fifo().revisit::<Page>().dedup())
    // A crawl that revisits never runs out of work: stop it with a time
    // limit or its shutdown token.
    .with_time_limit(Duration::from_hours(24))
    .with_seed(Seed::get("https://example.com/")?.with_tag(Page::Listing));
crawler.run().await?;
# Ok(())
# }
```

- A seed reported done or failed is crawled again its tag's interval later,
  on its first attempt again: a failed page may come back. Untagged seeds,
  tags outside the enum, and seeds skipped on purpose are crawled once.
- While only revisits wait, the frontier waits for the next one, so the
  crawl keeps running until it is stopped.
- `revisit` goes directly on the storage, which keeps the schedule, one
  revisit per page, so revisits come out of it like queued seeds: facades
  above it apply to them, and deduplication does not refuse them, as they
  are never pushed. Anywhere else, it does not compile.
- Intervals combine `d`, `h`, `m` and `s`, such as `"90s"` or `"1h30m"`, and
  are checked at compile time. An interval read at runtime, e.g. from a
  configuration file, takes a hand-written `impl Revisit`.

With a `RedbStore` frontier, revisits are kept in the store, so a crawl that
restarts picks up its schedule where it stopped:
`store.frontier().revisit::<Page>().dedup_with(store.seen())`. A crawl over
the store without `revisit` crawls the revisits that are due, and does not
wait for the others.

## Skipping unchanged pages

The `Revalidate` middleware sends conditional requests: for a page it has
seen, `If-None-Match` with its `ETag`, and `If-Modified-Since` with its
`Last-Modified`. A server answering `304 Not Modified` says the page did not
change, so its handlers are skipped:

```rust,no_run
use spire::backend::HttpClient;
use spire::frontier::FrontierExt;
use spire::redb::RedbStore;
use spire::worker::WorkerExt;
use spire::worker::middleware::Revalidate;
use spire::{Crawler, Result, Router};
# async fn page() {}

# async fn crawl() -> Result<()> {
let store = RedbStore::open("crawl.redb").await?;
let router = Router::new()
    .fallback(page)
    .layer(Revalidate::new(store.validators()));
let crawler = Crawler::new(HttpClient::default(), router)
    .with_frontier(store.frontier().dedup_with(store.seen()));

let stats = crawler.run().await?;
println!("{} pages unchanged", stats.unchanged);
# Ok(())
# }
```

- Validators are stored once a page's handlers continued, so a page whose
  extraction failed is fetched in full next time.
- `InMemValidators` keeps them for one run; `store.validators()` keeps them
  in the redb file, so later runs are conditional too.
- Only `GET` and `HEAD` requests are revalidated, and only over HTTP:
  browsers handle conditional requests themselves.
- `Stats::unchanged` counts the `304`s.

Together, revisits decide *when* a page is crawled again, and revalidation
makes that cheap when it did not change.
