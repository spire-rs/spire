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
`SqliteDataset<T>`, `SqliteSeen` and `SqliteValidators`. A store
uses a caller-owned connection pool; each named dataset stores JSON records and
reads them destructively in FIFO order. Batch operations are transactional,
including decoding: an incompatible record leaves the whole batch stored.

This crate depends on `spire-core`, keeping SQL drivers out of the crawl
engine. No backend is enabled by default. See the book's
[Feature flags](https://spire-rs.github.io/spire/features.html#sqlx-storage)
page for configuration.

## Usage

Enable `sqlite` on `spire` for `spire::sqlite`, or depend on `spire-sqlx`
with its `sqlite` feature directly:

```toml
[dependencies]
spire-sqlx = { version = "0.9.0", features = ["sqlite"] }
```

```rust,no_run
# #[cfg(feature = "sqlite")] {
use spire_core::dataset::Dataset;
use spire_sqlx::sqlite::SqliteStore;
use spire_sqlx::sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

# async fn example() -> Result<(), spire_core::BoxError> {
let pool = SqlitePoolOptions::new()
    .max_connections(1)
    .connect_with(
        SqliteConnectOptions::new()
            .filename("crawl.sqlite")
            .create_if_missing(true),
    )
    .await?;
let store = SqliteStore::new(pool.clone());
store.initialize().await?;
let titles = store.dataset::<String>("titles");
titles.write("Example Domain".into()).await?;
assert_eq!(titles.read().await?.as_deref(), Some("Example Domain"));
pool.close().await;
# Ok(())
# }
# }
```

Initialization applies embedded SQL migrations to the reserved `spire_*`
crawl tables, tracking versions and checksums in `_spire_sqlx_migrations`.
The application's `_sqlx_migrations` table and other schema remain untouched.
Schema changes and migration history commit together. Repeating initialization
is safe; changed applied migrations and unknown versions are rejected.
Pool, locking and durability options are never changed by the store.

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
