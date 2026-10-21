# Spire

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]
[![Crate Coverage][coverage-badge]][coverage-url]

**Check out other `spire` projects [here](https://github.com/spire-rs).**

[action-badge]: https://img.shields.io/github/actions/workflow/status/spire-rs/spire/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/spire-rs/spire/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/spire.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/spire
[docs-badge]: https://img.shields.io/docsrs/spire?logo=Docs.rs&style=flat-square
[docs-url]: http://docs.rs/spire
[coverage-badge]: https://img.shields.io/codecov/c/github/spire-rs/spire?logo=codecov&logoColor=white&style=flat-square
[coverage-url]: https://app.codecov.io/gh/spire-rs/spire

The flexible crawler & scraper framework powered by [tokio][tokio-rs/tokio] and
[tower][tower-rs/tower].

[tokio-rs/tokio]: https://github.com/tokio-rs/tokio/
[tower-rs/tower]: https://github.com/tower-rs/tower/

## Features

- **Tag-based routing**: requests carry a tag that picks their handler, with
  handlers written as async functions of typed extractors.
- **Declarative extraction**: parse pages with `Html`, or derive `Select` to
  turn CSS selectors into structs.
- **A real frontier**: deduplication, depth limits, retries with backoff and
  per-tag pauses that honour `Retry-After`.
- **Concurrent by default**: requests run concurrently up to a limit, and the
  crawl ends once nothing is queued or in flight.
- **Two backends**: plain HTTP through reqwest, and real browsers through
  WebDriver for pages that need JavaScript.
- **Tower all the way**: any tower middleware wraps routes, clients or
  backends.

## Installation

```toml
[dependencies]
spire = { version = "0.2.0", features = ["reqwest", "macros"] }
```

### Feature Flags

| Feature | Enables |
|---|---|
| `html` (default) | The `Html`, `Element` and `Elements` extractors and the `Select` trait |
| `macros` | `#[derive(Select)]` |
| `robots` | Obeying robots.txt (RFC 9309) with `Crawler::with_robots`, optionally fetched through a separate backend |
| `serde` | Serializable seeds for snapshots, and the `JsonlDataset` output |
| `redb` | `RedbStore`: a crash-safe frontier and seen set on disk |
| `reqwest` | The `HttpClient` backend |
| `thirtyfour` | The `BrowserBackend` (WebDriver) backend |
| `rustls` (default), `native-tls` | TLS for the enabled backends |
| `full` | Everything except `native-tls` |

## Quick Start

Crawl a listing, follow its pagination and extract every product:

```rust,no_run
# #[cfg(all(feature = "reqwest", feature = "macros"))]
# mod example {
use spire::dataset::InMemDataset;
use spire::extract::{Elements, Html, Select};
use spire::frontier::{FrontierExt, InMemFrontier, Link, Queue, Seed};
use spire::{Crawler, HttpClient, Result, Router};

#[derive(Debug, Select)]
#[select("article.product")]
struct Product {
    #[select("h2")]
    name: String,
    #[select(".price")]
    price: Option<f64>,
    #[select("a", attr = "href")]
    link: String,
}

/// Queues the next page and every product page of a listing.
async fn listing(queue: Queue, page: Html) -> Result<()> {
    let pages = page.links().into_iter().filter(|href| href.contains("?page="));
    queue.follow_all(pages).await?;
    for product in page.select_all::<Product>()? {
        queue.follow(Link::from(product.link).with_tag("product")).await?;
    }
    Ok(())
}

/// Stores the product on its own page.
async fn product(data: spire::dataset::Data<Product>, Elements(found): Elements<Product>) -> Result<()> {
    data.write_all(found).await
}

async fn crawl() -> Result<()> {
    let router = Router::new()
        .route("listing", listing)
        .route("product", product);

    let crawler = Crawler::new(HttpClient::default(), router)
        .with_frontier(InMemFrontier::fifo().dedup().max_depth(5).retry(3).tag_backoff())
        .with_dataset(InMemDataset::<Product>::new())
        .with_concurrency(16)
        .with_seed(Seed::get("https://example.com/products")?.with_tag("listing"));

    let stats = crawler.run().await?;
    let products = crawler.dataset::<Product>()?.read_all().await?;
    println!("{} products in {} requests", products.len(), stats.requests);
    Ok(())
}
# }
```

Relative links resolve against the page they were found on, after redirects;
followed links inherit the tag of their page unless given one. Error statuses
become failures: `5xx` and `429` are retried, honouring `Retry-After`.

### Browser Automation

`BrowserBackend` drives real browsers over WebDriver. Body extractors load
the page; `View` exposes the driver to interact with it:

```rust,no_run
# #[cfg(feature = "thirtyfour")]
# mod example {
use spire::extract::Html;
use spire::extract::driver::View;
use spire::frontier::Seed;
use spire::{BrowserBackend, Crawler, Result, Router};

async fn rendered(View(driver): View, page: Html) -> Result<()> {
    println!("{:?} at {:?}", page.title(), driver.current_url().await);
    Ok(())
}

async fn crawl() -> Result<()> {
    let backend = BrowserBackend::builder()
        .remote("http://localhost:4444")
        .max_sessions(4)
        .build()?;

    let router = Router::new().route("page", rendered);
    let crawler = Crawler::new(backend, router)
        .with_seed(Seed::get("https://example.com/")?.with_tag("page"));
    crawler.run().await?;

    // Quit the browser sessions instead of dropping them.
    crawler.backend().close().await;
    Ok(())
}
# }
```

## Architecture

- **`Crawler`** pops seeds from the frontier, runs them concurrently through
  the backend and the router, and reports each outcome back.
- **`Frontier`** holds the seeds; facades add deduplication, depth limits,
  retries and per-tag backoff.
- **`Backend`** hands out a client per request: `HttpClient` or a pooled
  `BrowserSession`.
- **`Router`** picks the handler for a request by its tag; handlers return
  anything that converts into an `Outcome`.
- **Datasets** store what handlers scrape, one per item type.

## Middleware

Routes are tower services, so tower middleware wraps them with
`Router::layer`; errors from middleware fail the request:

```rust
use std::time::Duration;

use spire::Router;

async fn handler() {}

let router: Router<spire::backend::Noop> = Router::new()
    .route("page", handler)
    .layer(tower::timeout::TimeoutLayer::new(Duration::from_secs(30)));
```

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](../CONTRIBUTING.md) for
guidelines.

## License

This project is licensed under the MIT License - see the
[LICENSE](../../LICENSE.txt) file for details.

## Acknowledgments

- Built on [tokio][tokio-rs/tokio] for async runtime
- Uses [tower][tower-rs/tower] for middleware composition
- Routing pattern inspired by [axum][tokio-rs/axum]

[tokio-rs/axum]: https://github.com/tokio-rs/axum/
