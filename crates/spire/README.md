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

The flexible crawler and scraper framework for Rust, built on
[tokio][tokio-rs/tokio].

[tokio-rs/tokio]: https://github.com/tokio-rs/tokio/

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

## Installation

```toml
[dependencies]
spire = { version = "0.2.0", features = ["reqwest", "macros"] }
```

### Feature Flags

| Feature | Enables |
|---|---|
| `html` (default) | The `Html` and `Element` extractors and the `Select` trait |
| `macros` | `#[derive(Select)]` |
| `robots` | Obeying robots.txt (RFC 9309) with `Crawler::with_robots`, optionally fetched through a separate backend |
| `sitemap` | The `Sitemap` extractor and `FollowSitemap` handler, for XML and text sitemaps, gzipped or not |
| `serde` | Serializable seeds for snapshots, and the `JsonlDataset` output |
| `redb` | `RedbStore`: a crash-safe frontier and seen set on disk |
| `reqwest` | The `HttpClient` backend |
| `thirtyfour` | The `BrowserBackend` (WebDriver) backend |
| `rustls-tls` (default), `native-tls` | TLS for the enabled backends |
| `full` | Everything except `native-tls` |

## Quick Start

Crawl a listing, follow its pagination and extract every product:

```rust,no_run
use spire::backend::HttpClient;
use spire::dataset::InMemDataset;
use spire::extract::Select;
use spire::frontier::FrontierExt;
use spire::frontier::memory::InMemFrontier;
use spire::frontier::seed::Seed;
use spire::request::Tags;
use spire::{Collect, Crawler, FollowLinks, Result, Router};

/// The kinds of page the crawl visits, routed by tag.
#[derive(Debug, Clone, Copy, Tags)]
enum Page {
    Listing,
    Product,
}

#[derive(Debug, Select)]
#[select("main.product")]
struct Product {
    #[select("h1")]
    name: String,
    #[select(".price")]
    price: Option<f64>,
}

async fn crawl() -> Result<()> {
    let router = Router::new()
        // Two handlers on one listing page, fetched once: follow the next
        // page and every product.
        .route(Page::Listing, (
            FollowLinks::new(Page::Listing).select("a.next"),
            FollowLinks::new(Page::Product).select("article.product a"),
        ))
        .route(Page::Product, Collect::<Product>::new())
        .require::<Page>();

    let crawler = Crawler::new(HttpClient::default(), router)
        .with_frontier(InMemFrontier::fifo().dedup().max_depth(5).retry(3).tag_backoff())
        .with_dataset(InMemDataset::<Product>::new())
        .with_concurrency(16)
        .with_seed(Seed::get("https://example.com/products")?.with_tag(Page::Listing));

    let stats = crawler.run().await?;
    let products = crawler.dataset::<Product>()?.read_all().await?;
    println!("{} products in {} requests", products.len(), stats.requests);
    Ok(())
}
```

`FollowLinks`, `Collect` and `FollowSitemap` cover the common jobs. Anything
else is an async function of extractors, e.g.
`async fn price(uri: Uri, Element(product): Element<Product>) -> Result<()>`,
and mixes freely with them in a tuple of up to 4 handlers. They run in turn
on the same page, and stop at the first that does not continue.

Relative links resolve against the page they were found on, after redirects.
Tags are not inherited: a followed link without one goes to the first
`route_path` pattern matching its path, such as `/products/*`, or else the
fallback. `require` fails at startup if a variant has no route, and a handler
serving several variants takes `page: Page` as an argument. A URL is fetched
once whatever its tag. Error statuses become failures: `5xx` and `429` are
retried, honouring `Retry-After`.

### Sitemaps

With the `sitemap` feature, the `FollowSitemap` handler follows sitemaps:
every page is queued with one tag, and every sitemap of an index with the tag
of the sitemap itself. Seed sitemaps yourself, or let `Robots::with_sitemaps`
queue the ones each site's robots.txt lists:

```rust,no_run
use spire::backend::HttpClient;
use spire::frontier::FrontierExt;
use spire::frontier::memory::InMemFrontier;
use spire::frontier::seed::Seed;
use spire::request::Tags;
use spire::{Crawler, FollowSitemap, Result, Robots, Router};

#[derive(Debug, Clone, Copy, Tags)]
enum Page {
    Home,
    Sitemap,
    Product,
}

async fn product() {}

async fn crawl() -> Result<()> {
    let router = Router::new()
        .route(Page::Home, || async {})
        .route(Page::Sitemap, FollowSitemap::new(Page::Product))
        .route(Page::Product, product);

    let crawler = Crawler::new(HttpClient::default(), router)
        .with_frontier(InMemFrontier::fifo().dedup().allow_hosts(["example.com"]))
        .with_robots(Robots::new("MyBot").with_sitemaps(Page::Sitemap))
        .with_seed(Seed::get("https://example.com/")?.with_tag(Page::Home));
    crawler.run().await?;
    Ok(())
}
```

The `Sitemap` extractor parses a sitemap for a handler of your own. Sitemaps
must be fetched over HTTP: a browser renders XML as a page.

### Browser Automation

`BrowserBackend` drives real browsers over WebDriver BiDi, with the page's
real status and headers, any request method, and a fresh browser profile per
request. The page is loaded once, by the first extractor that needs it, and
body extractors such as `Html` read it as currently rendered. `View` hands
over the driver to interact with the page; `view.snapshot()` then parses it again,
including what the interaction changed:

```rust,no_run
use spire::backend::BrowserBackend;
use spire::backend::thirtyfour::By;
use spire::extract::View;
use spire::frontier::queue::Queue;
use spire::frontier::seed::Seed;
use spire::{Crawler, Result, Router};

async fn rendered(queue: Queue, view: View) -> Result<()> {
    if let Ok(more) = view.find(By::Css("button.more")).await {
        let _ = more.click().await;
    }
    let page = view.snapshot().await?;
    println!("{:?} at {:?}", page.title(), view.current_url().await);
    queue.follow_all(page.links()).await?;
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

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](../CONTRIBUTING.md) for
guidelines.

## License

This project is licensed under the MIT License; see the
[LICENSE](../../LICENSE) file for details.

## Acknowledgments

- Built on [tokio][tokio-rs/tokio] for async runtime
- Routing pattern inspired by [axum][tokio-rs/axum]

[tokio-rs/axum]: https://github.com/tokio-rs/axum/
