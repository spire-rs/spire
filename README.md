<div align="center">

<img src=".github/assets/logo.png" alt="Spire" width="104" height="104" />

# Spire

**The flexible crawler and scraper framework for Rust.**

Crawl websites over HTTP or in real browsers, route every page to a typed
handler, and extract structured data, built on tokio.

[![Build](https://img.shields.io/github/actions/workflow/status/spire-rs/spire/build.yml?branch=main&label=build&style=flat-square)](https://github.com/spire-rs/spire/actions/workflows/build.yml)
[![Security](https://img.shields.io/github/actions/workflow/status/spire-rs/spire/security.yml?branch=main&label=security&style=flat-square)](https://github.com/spire-rs/spire/actions/workflows/security.yml)
[![Crates.io](https://img.shields.io/crates/v/spire.svg?style=flat-square)](https://crates.io/crates/spire)
[![Docs](https://img.shields.io/docsrs/spire?style=flat-square)](https://docs.rs/spire)

[**The Spire Book**](https://spire-rs.github.io/spire) · [**docs.rs/spire**](https://docs.rs/spire) · [**crates.io/crates/spire**](https://crates.io/crates/spire) · [**github.com/spire-rs**](https://github.com/spire-rs)

</div>

A crawl flows through three parts: the **frontier** queues the requests to make
and decides what is new, the **crawler** fetches them concurrently through a
**backend** (HTTP or a browser) while keeping to per-host limits and
robots.txt, and the **router** hands each response to a handler. Handlers are
async functions whose arguments are extractors: parsed HTML, typed JSON, the
queue for links to follow, or a dataset to store results in.

> [!WARNING]
> **Active development. API not stable.** Public APIs and on-disk formats may
> change without notice between releases. Pin a specific version if you depend
> on this in production.

## Features

- **HTTP and browser backends**: reqwest for plain HTTP, thirtyfour to drive real browsers over WebDriver.
- **Typed routing**: route by tags, checked by the compiler with `#[derive(Tags)]`, or by URL path patterns.
- **Extractors**: parse HTML into structs with `#[derive(Select)]`, deserialize JSON, or drive the browser with `Tab`.
- **Polite crawling**: per-host concurrency and delays, robots.txt per RFC 9309, retries with `Retry-After` and backoff.
- **Sitemaps**: follow XML and text sitemaps and indexes, gzipped or not, found through robots.txt or seeded directly.
- **Frontiers**: deduplication with URL normalization, depth limits, host filters, priorities, and a Bloom filter for huge crawls.
- **Crash safety**: a redb-backed frontier and seen set that resume a crawl after a crash.

## Requirements

- **Rust + Cargo**: 1.95+, Edition 2024
- **A WebDriver server**, such as chromedriver, for the browser backend only.

## Quick start

Add spire with the HTTP backend, and tokio:

```toml
[dependencies]
spire = { version = "0.8", features = ["reqwest"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

Crawl a site two links deep and print every title:

```rust
use spire::backend::ReqwestBackend;
use spire::extract::html::{Html, Links};
use spire::frontier::queue::Queue;
use spire::frontier::seed::Seed;
use spire::frontier::{FrontierExt, InMemFrontier};
use spire::{Crawler, Result, Router};

/// Prints the title of every page and follows its links.
async fn page(queue: Queue, page: Html, links: Links) -> Result<()> {
    println!("{:?}", page.title());
    queue.follow_all(links).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let router = Router::new().route_path("/**", page);
    let frontier = InMemFrontier::fifo()
        .dedup()
        .max_depth(2)
        .allow_hosts(["example.com"]);

    let crawler = Crawler::new(ReqwestBackend::default(), router)
        .with_frontier(frontier)
        .with_seed(Seed::get("https://example.com/")?);
    crawler.run().await?;
    Ok(())
}
```

[The Spire Book](https://spire-rs.github.io/spire) is the guide: handling pages, discovering
them, fetching over HTTP or with a browser, running and resuming a crawl, and extending spire.

See the book's [Installation](https://spire-rs.github.io/spire/installation.html)
and [Feature flags](https://spire-rs.github.io/spire/features.html) pages for
configuration, or [examples/](examples/README.md) for runnable crawls.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines, and
[CHANGELOG.md](CHANGELOG.md) for release notes.

## License

MIT License, see [LICENSE](LICENSE).
