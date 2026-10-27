# spire-redb

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]
[![Crate Coverage][coverage-badge]][coverage-url]

**Check out other `spire` projects [here](https://github.com/spire-rs).**

[action-badge]: https://img.shields.io/github/actions/workflow/status/spire-rs/spire/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/spire-rs/spire/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/spire-redb.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/spire-redb
[docs-badge]: https://img.shields.io/docsrs/spire-redb?logo=Docs.rs&style=flat-square
[docs-url]: http://docs.rs/spire-redb
[coverage-badge]: https://img.shields.io/codecov/c/github/spire-rs/spire?logo=codecov&logoColor=white&style=flat-square
[coverage-url]: https://app.codecov.io/gh/spire-rs/spire
Crash-safe frontier and seen set for Spire, stored in
[redb](https://github.com/cberner/redb), a pure-Rust embedded database.

## Overview

A `RedbStore` is one database file holding a crawl's queue, the URLs it
has seen, and what it scraped:

- **`RedbFrontier`** queues seeds by priority, then in insertion order. A
  popped seed stays recorded as in flight until it is reported, so the seeds
  in flight at a crash are queued again when the store is reopened and
  crawled again: requests are at least once, never lost.
- **`RedbSeen`** deduplicates exactly, on disk, so memory stays flat however
  large the crawl.
- **`RedbDataset`** stores a crawl's results as a table per dataset, as
  JSON, read back in the order they were written. Items survive a crash as
  the frontier does; a page whose items were committed just before a crash
  may be crawled again and write them twice.

Every change is a committed transaction. Commits are synced to disk at most
once a second: a crash rolls the store back to its last synced commit, a
consistent earlier state, so it repeats at most a second of work and loses
none, since a link is never committed after the page that found it.
`RedbStore::builder().sync_always()` syncs every commit instead, repeating
only the requests in flight, at a large cost in throughput. In the `redb`
benchmark on macOS, pushing and popping a seed takes about 0.6 ms with batched
syncs and about 11 ms with `sync_always`, twenty times slower; run
`make bench ARGS=redb` to measure your own machine.

## Feature flags

This crate has no feature flags. Enable `redb` on `spire` to use its
re-exports, or depend on `spire-redb` directly.

## Usage

```rust,no_run
use spire_core::backend::Noop;
use spire_core::frontier::FrontierExt;
use spire_core::frontier::seed::Seed;
use spire_core::worker::from_fn;
use spire_core::crawler::Crawler;
use spire_core::Context;
use spire_redb::RedbStore;

async fn example() -> spire_core::Result<()> {
    let store = RedbStore::open("crawl.redb").await?;
    let frontier = store.frontier().dedup_with(store.seen()).retry(3);

    let worker = from_fn(|cx: Context<Noop>| async move {
        cx.dataset::<String>()?.write(cx.request().uri().to_string()).await
    });
    let crawler = Crawler::new(Noop, worker)
        .with_frontier(frontier)
        .with_dataset(store.dataset::<String>("pages"))
        .with_seed(Seed::get("https://example.com/")?);

    // After a crash, opening the same file resumes where the crawl stopped;
    // seeds pushed again are refused as already seen.
    crawler.run().await?;
    Ok(())
}
```

## Contributing

This crate follows the same contribution guidelines as the main spire project.
Please see [CONTRIBUTING.md](../../CONTRIBUTING.md) for details.

## License

This project is licensed under the MIT License; see the
[LICENSE](../../LICENSE) file for details.
