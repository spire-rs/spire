# spire-core

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]

**Check out other `spire` projects [here](https://github.com/spire-rs).**

[action-badge]: https://img.shields.io/github/actions/workflow/status/spire-rs/spire/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/spire-rs/spire/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/spire-core.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/spire-core
[docs-badge]: https://img.shields.io/docsrs/spire-core?logo=Docs.rs&style=flat-square
[docs-url]: http://docs.rs/spire-core

The crawl engine behind [`spire`](https://crates.io/crates/spire): the
crawler, its frontier, backends, workers and datasets.

## Overview

- A [`Backend`] hands out a [`Session`] per request: an HTTP client, a pooled
  browser session, or anything else that turns a request into a response.
- A [`Worker`] handles each request and returns an [`Outcome`]: continue,
  retry, back off, stop or fail.
- The [`Frontier`] holds the [`Seed`]s waiting to be crawled. Facades add
  deduplication, a depth limit, retries and per-tag backoff on top of any
  storage.
- The [`Crawler`] runs requests concurrently until the frontier is empty and
  nothing is in flight.

Crawls are bounded and polite: crawl scope (`allow_hosts`, `filter`), request
and time budgets, per-request deadlines, per-host concurrency and delays, and
robots.txt with the `robots` feature.

## Installation

Most users depend on `spire`, which adds a router, handlers and extractors on
top of this crate. Depend on `spire-core` directly to implement a backend, a
frontier or a dataset:

```toml
[dependencies]
spire-core = { version = "0.11", features = ["robots"] }
```

No feature is enabled by default:

| Feature | Enables |
|---|---|
| `robots` | Fetching and obeying robots.txt, see `Crawler::with_robots` |
| `serde` | Serializable seeds, fingerprints and Bloom filters, for snapshots |
| `json` | `JsonlDataset`, writing results as JSON Lines |
| `csv` | `CsvDataset`, writing results as CSV |
| `testing` | `MockSite` and `Visit`, to test handlers without a network |
| `user-agents` | Browser user agents with matching navigation headers |

## Usage

```rust
use spire_core::backend::Noop;
use spire_core::frontier::seed::Seed;
use spire_core::frontier::{FrontierExt, InMemFrontier};
use spire_core::worker::{Outcome, from_fn};
use spire_core::crawler::Crawler;
use spire_core::{Context, Result};

async fn handle(cx: Context<Noop>) -> Result<Outcome> {
    let queue = cx.queue().clone();
    let response = cx.response().await?;
    if response.status().as_u16() == 429 {
        return Ok(Outcome::backoff(std::time::Duration::from_secs(30)));
    }

    queue.follow("https://example.com/next").await?;
    Ok(Outcome::Continue)
}

async fn example() -> Result<()> {
    let frontier = InMemFrontier::fifo()
        .dedup()
        .max_depth(3)
        .retry(3)
        .tag_backoff();

    let stats = Crawler::new(Noop, from_fn(handle))
        .with_frontier(frontier)
        .with_concurrency(16)
        .with_seed(Seed::get("https://example.com/")?)
        .run()
        .await?;

    println!("crawled {} requests in {:?}", stats.requests, stats.elapsed);
    Ok(())
}
```

## Learn more

- The book's [Custom backends](https://spire-rs.github.io/spire/custom-backends.html)
  and [Custom storage](https://spire-rs.github.io/spire/custom-storage.html)
  pages, and [Feature flags](https://spire-rs.github.io/spire/features.html#using-spire-core-directly)
- The [API documentation](https://docs.rs/spire-core)

## Contributing

See [CONTRIBUTING.md](../../CONTRIBUTING.md) for development setup and
guidelines.

## License

MIT License, see [LICENSE](../../LICENSE).

[`Backend`]: https://docs.rs/spire-core/latest/spire_core/backend/trait.Backend.html
[`Session`]: https://docs.rs/spire-core/latest/spire_core/backend/trait.Session.html
[`Worker`]: https://docs.rs/spire-core/latest/spire_core/worker/trait.Worker.html
[`Outcome`]: https://docs.rs/spire-core/latest/spire_core/worker/enum.Outcome.html
[`Frontier`]: https://docs.rs/spire-core/latest/spire_core/frontier/trait.Frontier.html
[`Seed`]: https://docs.rs/spire-core/latest/spire_core/frontier/seed/struct.Seed.html
[`Crawler`]: https://docs.rs/spire-core/latest/spire_core/crawler/struct.Crawler.html
