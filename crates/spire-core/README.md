# spire-core

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]
[![Crate Coverage][coverage-badge]][coverage-url]

**Check out other `spire` projects [here](https://github.com/spire-rs).**

[action-badge]: https://img.shields.io/github/actions/workflow/status/spire-rs/spire/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/spire-rs/spire/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/spire-core.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/spire-core
[docs-badge]: https://img.shields.io/docsrs/spire-core?logo=Docs.rs&style=flat-square
[docs-url]: http://docs.rs/spire-core
[coverage-badge]: https://img.shields.io/codecov/c/github/spire-rs/spire?logo=codecov&logoColor=white&style=flat-square
[coverage-url]: https://app.codecov.io/gh/spire-rs/spire

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
robots.txt with the `robots` feature. With the `serde` feature, seeds are
serializable, so a stopped crawl's frontier and seen set can be snapshotted
and restored. `JsonlDataset` and `CsvDataset`, with the `json` and `csv`
features, write results to files.

## Feature flags

No features are enabled by default.

| Feature | Enables |
|---|---|
| `robots` | Fetching and obeying robots.txt with `Crawler::with_robots` |
| `serde` | Serialization of `Seed`, `Fingerprint` and `BloomSnapshot` |
| `json` | `JsonlDataset`, writing JSON Lines; also enables `serde` |
| `csv` | `CsvDataset`, writing CSV; also enables `serde` |
| `testing` | `MockSite`, `MockPage`, `MockSession` and `Visit` for offline tests |
| `user-agents` | `Identity::with_profile` and the uagent `Profile`, `UserAgents`, `Browser` and `Platform` re-exports |

Most users depend on `spire`, which adds a router, handlers and extractors on
top of this crate. Depend on `spire-core` directly to implement a backend, a
frontier or a dataset.

## Example

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

[`Backend`]: https://docs.rs/spire-core/latest/spire_core/backend/trait.Backend.html
[`Session`]: https://docs.rs/spire-core/latest/spire_core/backend/trait.Session.html
[`Worker`]: https://docs.rs/spire-core/latest/spire_core/worker/trait.Worker.html
[`Outcome`]: https://docs.rs/spire-core/latest/spire_core/worker/enum.Outcome.html
[`Frontier`]: https://docs.rs/spire-core/latest/spire_core/frontier/trait.Frontier.html
[`Seed`]: https://docs.rs/spire-core/latest/spire_core/frontier/struct.Seed.html
[`Crawler`]: https://docs.rs/spire-core/latest/spire_core/crawler/struct.Crawler.html
