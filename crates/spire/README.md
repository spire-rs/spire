# Spire

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]
[![Book][book-badge]][book-url]

**Check out other `spire` projects [here](https://github.com/spire-rs).**

[action-badge]: https://img.shields.io/github/actions/workflow/status/spire-rs/spire/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/spire-rs/spire/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/spire.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/spire
[docs-badge]: https://img.shields.io/docsrs/spire?logo=Docs.rs&style=flat-square
[docs-url]: http://docs.rs/spire
[book-badge]: https://img.shields.io/badge/book-guide-blue?logo=mdbook&style=flat-square
[book-url]: https://spire-rs.github.io/spire

The flexible crawler and scraper framework for Rust, built on
[tokio][tokio-rs/tokio].

[tokio-rs/tokio]: https://github.com/tokio-rs/tokio/

## Features

- **Tag-based routing**: requests carry a tag that picks their handler, with
  handlers written as async functions of typed extractors.
- **Declarative extraction**: parse pages with `Html`, or derive `Select` to
  turn CSS selectors into structs.
- **A real frontier**: deduplication, depth limits, and retries with
  backoff.
- **Polite by default**: hosts that ask to slow down get wider gaps, and a
  `Retry-After` pauses the host for as long as it asks.
- **Concurrent by default**: requests run concurrently up to a limit, and the
  crawl ends once nothing is queued or in flight.
- **Two backends**: plain HTTP through reqwest, and real browsers through
  WebDriver for pages that need JavaScript.

## Installation

```toml
[dependencies]
spire = { version = "0.8.0", features = ["reqwest", "macros"] }
```

See the book’s [Feature flags](https://spire-rs.github.io/spire/features.html) page for configuration.

## Quick Start

Crawl a listing, follow its pagination and extract every product:

```rust,no_run
use spire::backend::ReqwestBackend;
use spire::dataset::InMemDataset;
use spire::extract::html::Select;
use spire::frontier::seed::Seed;
use spire::frontier::{FrontierExt, InMemFrontier};
use spire::handler::{Collect, FollowLinks};
use spire::crawler::Tags;
use spire::{Crawler, Result, Router};

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

    let crawler = Crawler::new(ReqwestBackend::default(), router)
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

## Guide

[The Spire Book](https://spire-rs.github.io/spire) explains every part of a crawl:

- **Getting started**: [installation](https://spire-rs.github.io/spire/installation.html), a
  [first crawl](https://spire-rs.github.io/spire/first-crawl.html), and
  [how a crawl works](https://spire-rs.github.io/spire/concepts.html)
- **Handling pages**: [routing](https://spire-rs.github.io/spire/routing.html),
  [extractors](https://spire-rs.github.io/spire/extractors.html),
  [state](https://spire-rs.github.io/spire/state.html),
  [HTML](https://spire-rs.github.io/spire/html.html), and [storing results](https://spire-rs.github.io/spire/datasets.html)
- **Discovering pages**: [following links](https://spire-rs.github.io/spire/links.html),
  [best-first crawling](https://spire-rs.github.io/spire/best-first.html),
  [sitemaps](https://spire-rs.github.io/spire/sitemaps.html), and
  [avoiding traps](https://spire-rs.github.io/spire/avoiding-traps.html)
- **Fetching**: [HTTP](https://spire-rs.github.io/spire/http.html), [browsers](https://spire-rs.github.io/spire/browsers.html),
  [middleware](https://spire-rs.github.io/spire/middleware.html), and
  [identities](https://spire-rs.github.io/spire/identities.html)
- **Running a crawl**: [the frontier](https://spire-rs.github.io/spire/frontier.html),
  [errors and retries](https://spire-rs.github.io/spire/errors.html),
  [limits and politeness](https://spire-rs.github.io/spire/politeness.html),
  [recrawls](https://spire-rs.github.io/spire/recrawls.html), [crash safety](https://spire-rs.github.io/spire/redb.html), and
  [logs and statistics](https://spire-rs.github.io/spire/observability.html)
- **Extending spire**: [testing](https://spire-rs.github.io/spire/testing.html),
  [custom backends](https://spire-rs.github.io/spire/custom-backends.html), and
  [custom storage](https://spire-rs.github.io/spire/custom-storage.html)

The API reference is on [docs.rs](https://docs.rs/spire).

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
