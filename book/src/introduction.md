# Introduction

Spire is a crawler and scraper framework for Rust, built on
[tokio](https://tokio.rs). You write what to do with each kind of page, and
Spire runs the crawl around it: the queue of pages to visit, fetching them
concurrently and politely, retrying what fails, and stopping once there is
nothing left to do.

```rust,no_run
use spire::backend::ReqwestBackend;
use spire::extract::html::{Html, Links};
use spire::frontier::queue::Queue;
use spire::frontier::seed::Seed;
use spire::frontier::{FrontierExt, InMemFrontier};
use spire::{Crawler, Result, Router};

/// Prints the title of every page and follows its links.
async fn page(html: Html, links: Links, queue: Queue) -> Result<()> {
    println!("{}", html.title().unwrap_or_default());
    queue.follow_all(links).await?;
    Ok(())
}

# async fn crawl() -> Result<()> {
let router = Router::new().fallback(page);
// Each URL once, two links deep, on one site.
let frontier = InMemFrontier::fifo()
    .dedup()
    .max_depth(2)
    .allow_hosts(["example.com"]);
let crawler = Crawler::new(ReqwestBackend::default(), router)
    .with_frontier(frontier)
    .with_seed(Seed::get("https://example.com/")?);
crawler.run().await?;
# Ok(())
# }
```

Handlers are async functions, and their arguments are *extractors*: typed
values taken from the request or the page, such as its parsed HTML or the
queue to add links to. If you have used [axum](https://github.com/tokio-rs/axum),
this will feel familiar.

## What Spire does for you

- **Routing**: every request carries a tag, such as `listing` or `product`,
  that picks its handler. Tags can be an enum, checked by the compiler.
- **Extraction**: parse pages into structs with CSS selectors through
  `#[derive(Select)]`, or read JSON, text, metadata, JSON-LD or Markdown.
  With the `agent` feature, a language model extracts typed data instead.
- **Discovery**: follow links, sitemaps or both, with deduplication, depth
  limits and host filters, and crawl the most promising pages first.
- **Politeness**: concurrency and delays per host that slow down when a host
  is throttled, `Retry-After`, robots.txt, and budgets of requests and time.
- **Reliability**: retries with backoff, and with redb or SQLite, a crawl that
  survives a crash and resumes where it stopped, revisiting pages on a
  schedule.
- **Two backends**: plain HTTP through reqwest, and real browsers over
  WebDriver for pages that need JavaScript.
- **Middleware, identities and testing**: change every request, send each as
  one consistent visitor with its own proxy, user agent and cookies, and test
  handlers against fixed pages without a network.

## The crates

Most projects depend only on `spire`, which re-exports the others behind
feature flags:

| Crate | Contains | Feature of `spire` |
|---|---|---|
| `spire` | The router, handlers and extractors, and everything below | |
| `spire-core` | The crawler, frontier, backend traits, datasets and middleware | Always |
| `spire-macros` | `#[derive(Select)]`, `#[derive(Tags)]`, `#[derive(Revisit)]` and `#[derive(FromRef)]` | `macros` |
| `spire-reqwest` | `ReqwestBackend`, the HTTP backend | `reqwest` |
| `spire-thirtyfour` | `ThirtyfourBackend`, the WebDriver backend | `thirtyfour` |
| `spire-redb` | `RedbStore`, a crawl kept in one redb file | `redb` |
| `spire-sqlx` | `SqliteStore`, a crawl kept in a SQLite database | `sqlite` |
| `spire-agent` | Model extraction, its budgets and caches | `agent` |

Depend on `spire-core` directly only to implement your own backend, frontier
or dataset; see [Extending Spire](custom-backends.md).

## How this book is organised

- **Getting started** installs Spire, builds a first crawl, and explains how
  the parts of a crawl fit together.
- **Writing handlers** and **Extracting data** cover routing, extractors,
  state, testing, HTML and where results go.
- **AI-driven crawling** extracts data and picks links with a language model.
- **Controlling discovery** decides which pages are crawled, and in what
  order.
- **Fetching pages** covers the HTTP and browser backends, identities and
  middleware.
- **Running reliably** covers politeness, errors, recrawls, persistence and
  observing a crawl.
- **Extending Spire** implements your own backend or storage.

The API reference is on [docs.rs](https://docs.rs/spire). This book is the
guide: it explains how the pieces fit together, chapter by chapter.
