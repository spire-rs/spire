# Introduction

Spire is a crawler and scraper framework for Rust, built on
[tokio](https://tokio.rs). You describe what to do with each kind of page,
and Spire handles the rest of the crawl: the queue of pages to visit,
fetching them concurrently and politely, retrying what fails, and stopping
once there is nothing left to do.

```rust,no_run
use spire::backend::HttpClient;
use spire::extract::Html;
use spire::frontier::queue::Queue;
use spire::frontier::seed::Seed;
use spire::{Crawler, Result, Router};

async fn page(html: Html, queue: Queue) -> Result<()> {
    println!("{}", html.title().unwrap_or_default());
    queue.follow_all(html.links()).await?;
    Ok(())
}

# async fn crawl() -> Result<()> {
let router = Router::new().fallback(page);
let crawler = Crawler::new(HttpClient::default(), router)
    .with_seed(Seed::get("https://example.com/")?);
crawler.run().await?;
# Ok(())
# }
```

Handlers are async functions, and their arguments are *extractors*: typed
values taken from the page, such as its parsed HTML or the queue to add
links to. If you have used [axum](https://github.com/tokio-rs/axum), this
will feel familiar.

## What Spire does for you

- **Routing**: every request carries a tag, such as `listing` or
  `product`, that picks its handler. Tags can be an enum, checked by the
  compiler.
- **Extraction**: parse pages into structs with CSS selectors, using
  `#[derive(Select)]`, or read JSON, text and raw bytes.
- **A real frontier**: deduplication, depth limits, retries with backoff,
  and pauses that honour `Retry-After`. With redb, the crawl survives a
  crash and resumes where it stopped.
- **Politeness**: per-host concurrency and delays, robots.txt, and limits on
  requests, time and each request's duration.
- **Two backends**: plain HTTP through reqwest, and real browsers over
  WebDriver for pages that need JavaScript.
- **Middleware and testing**: change every request, rotate user agents and
  proxies, and test handlers against fixed pages without a network.

## The crates

Most projects depend only on `spire`, which re-exports the rest behind
feature flags:

| Crate | Contains |
|---|---|
| `spire` | The router, handlers and extractors, and everything below |
| `spire-core` | The crawler, frontier, backend traits, datasets and middleware |
| `spire-macros` | `#[derive(Select)]` and `#[derive(Tags)]` |
| `spire-reqwest` | `HttpClient`, the HTTP backend |
| `spire-thirtyfour` | `BrowserBackend`, the WebDriver backend |
| `spire-redb` | `RedbStore`, a crash-safe frontier on disk |

The API reference is on [docs.rs](https://docs.rs/spire). This book is the
guide: it explains how the pieces fit together, chapter by chapter.
