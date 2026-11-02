# spire-sqlx

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]

**Check out other `spire` projects [here](https://github.com/spire-rs).**

[action-badge]: https://img.shields.io/github/actions/workflow/status/spire-rs/spire/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/spire-rs/spire/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/spire-sqlx.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/spire-sqlx
[docs-badge]: https://img.shields.io/docsrs/spire-sqlx?logo=Docs.rs&style=flat-square
[docs-url]: http://docs.rs/spire-sqlx

SQLx-backed crawl storage for Spire, with independently enabled database backends.

## Overview

The `sqlite` module provides `SqliteStore`, `SqliteFrontier`,
`SqliteDataset<T>`, `SqliteSeen` and `SqliteValidator`. A store opens its
own database file, or uses a connection pool you configured; each named dataset
stores JSON records and reads them destructively in FIFO order. Batch operations are transactional,
including decoding: an incompatible record leaves the whole batch stored.

The frontier serializes seeds when enqueueing or scheduling them. Caller-attached
seed extensions are discarded even within the same process; register runtime
services with `Crawler::with_extension` instead.

This crate depends on `spire-core`, keeping SQL drivers out of the crawl
engine. No backend is enabled by default. See the book's
[Feature flags](https://spire-rs.github.io/spire/features.html#sqlx-storage)
page for configuration.

## Usage

Enable `sqlite` on `spire` for `spire::store::SqliteStore`, or depend on `spire-sqlx`
with its `sqlite` feature directly:

```toml
[dependencies]
spire-sqlx = { version = "0.9.0", features = ["sqlite"] }
```

```rust,no_run
# #[cfg(feature = "sqlite")] {
use spire_core::dataset::Dataset;
use spire_sqlx::sqlite::SqliteStore;

# async fn example() -> Result<(), spire_core::BoxError> {
let store = SqliteStore::open("crawl.sqlite").await?;
let titles = store.dataset::<String>("titles");
titles.write("Example Domain".into()).await?;
assert_eq!(titles.read().await?.as_deref(), Some("Example Domain"));
store.close().await;
# Ok(())
# }
# }
```

`SqliteStore::open` uses write-ahead logging, `synchronous=NORMAL`, a five-second
busy timeout and one connection; `SqliteStore::builder` changes all but the
first. WAL is saved in the file, so opening an existing database converts it.
`SqliteStore::new` instead uses a pool you created, whose options the store
never changes, followed by `store.initialize().await?`.

Opening or initializing applies embedded SQL migrations to the reserved `spire_*`
crawl tables, tracking versions and checksums in `_spire_sqlx_migrations`.
The application's `_sqlx_migrations` table and other schema remain untouched.
Schema changes and migration history commit together. Repeating this is safe;
changed applied migrations and unknown versions are rejected.

`store.frontier().await?` acquires exclusive ownership through a sidecar
file lock and requeues abandoned in-flight requests. Clones share ownership;
other stores or processes cannot open a frontier until it is released.
Datasets, seen sets, validators and caches remain shareable across pools.

See [Persistence with SQLite](https://spire-rs.github.io/spire/sqlite.html) for pool
configuration, concurrency, rollback and persistence guarantees.

## Contributing

See [CONTRIBUTING.md](../../CONTRIBUTING.md) for development guidelines.

## License

This project is licensed under the MIT License; see
[LICENSE](../../LICENSE) for details.
