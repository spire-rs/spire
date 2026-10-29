# Logs and statistics

## Logs

Spire logs through [tracing](https://docs.rs/tracing), with one target per
crate, so each can be turned up or down on its own:

| Target | Logs |
|---|---|
| `spire_core` | The crawl: starting and stopping, requests failing or retried, hosts slowed down or paused, robots.txt, revisits |
| `spire` | Routing and the built-in handlers: links followed, items collected, sitemaps |
| `spire_redb` | The redb store: unreadable seeds, pending changes |
| `spire_thirtyfour` | Browser sessions starting and quitting |

At `info` and above, a crawl logs what an operator wants to see: the crawl
starting and stopping, tags aborted, hosts paused for `Retry-After`, and
robots.txt it could not fetch. Requests that fail for good are `warn`.
Every request and every decision, such as a host's concurrency or gap
changing, is `debug`.

Install a subscriber to see them, e.g. with `tracing-subscriber` and its
`env-filter` feature:

```rust,ignore
tracing_subscriber::fmt()
    .with_env_filter("spire_core=info,spire=info")
    .init();
```

Each request runs in a `request` span with its method, URI, tag, depth and
attempt, so a subscriber that prints spans shows which request a line
belongs to.

## Statistics

`run` returns `Stats` once the crawl is over, and `stats()` returns them
while it runs, e.g. to report progress from another task:

| Field | Counts |
|---|---|
| `requests` | Requests handled, every attempt counted |
| `succeeded` | Requests that finished with `Continue` or `Stop` |
| `failed` | Requests that failed for good |
| `retried` | Requests the frontier took back for another attempt |
| `skipped` | Requests not crawled on purpose: robots.txt, an aborted tag |
| `cancelled` | Requests still in flight when a shutdown grace period ended |
| `unchanged` | Pages the server answered `304 Not Modified`, see [Recrawls](recrawls.md) |
| `elapsed` | How long the crawl has run |

```rust,no_run
use std::sync::Arc;
use std::time::Duration;

use spire::backend::ReqwestBackend;
use spire::{Crawler, Result, Router};
# async fn page() {}

# async fn crawl() -> Result<()> {
let crawler = Arc::new(Crawler::new(ReqwestBackend::default(), Router::new().fallback(page)));

let progress = Arc::clone(&crawler);
tokio::spawn(async move {
    loop {
        tokio::time::sleep(Duration::from_secs(10)).await;
        let stats = progress.stats();
        println!("{} requests, {} failed", stats.requests, stats.failed);
    }
});

let stats = crawler.run().await?;
println!("done in {:?}", stats.elapsed);
# Ok(())
# }
```

To follow a crawl event by event instead, see [Crawl events](events.md).
