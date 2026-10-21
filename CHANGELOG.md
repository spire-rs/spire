# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-10-21

The first working release; 0.1.0 only reserved the crate names. Requires
Rust 1.95.

### Added

- `spire-core`: the `Crawler` runs requests concurrently, with per-host
  concurrency and delays, request and crawl budgets
  (`with_max_requests`, `with_time_limit`), per-request timeouts, graceful
  shutdown, and live `Stats`. Handlers steer it with an `Outcome`: continue,
  retry, back off a tag (honouring `Retry-After`), abort a tag, or stop.
- `spire-core`: frontiers queue `Seed`s in FIFO, LIFO or priority order, and
  facades chained with `FrontierExt` add host filters, deduplication over
  normalized URLs, depth limits, retries with jitter, and per-tag backoff.
  Seen sets are exact (`InMemSeen`) or fixed-memory (`BloomSeen`).
- `spire-core`: robots.txt per RFC 9309 (`robots` feature), fetched once per
  origin, honouring `Crawl-delay` and retrying unreachable files.
- `spire-core`: datasets collect scraped items by type (`InMemDataset`, and
  `JsonlDataset` with the `serde` feature, which also makes seeds and
  frontier snapshots serializable). Error statuses follow a shared
  `StatusPolicy`. Every handled request is logged at `debug`.
- `spire`: a `Router` routes by tag, with typed tags from
  `#[derive(Tags)]` checked by `Router::require`, by path pattern for
  untagged requests, or to a fallback. Handlers are async functions of
  extractors: the URI, tag, depth and attempt, the `Queue` for links to
  follow, datasets, shared `State`, and the body as `Bytes`, `Text`, `Json`,
  `Html`, or structs extracted with `#[derive(Select)]` through `Element`.
- `spire-reqwest`: `HttpClient`, an HTTP backend with HTTP/2, system proxies,
  compression (`gzip`, `brotli`, `zstd`, `deflate`), `cookies`, `socks` and
  `hickory-dns` features, and TLS through `rustls-tls` or `native-tls`.
- `spire-thirtyfour`: `BrowserBackend` crawls with real browsers over
  WebDriver BiDi: pooled sessions across remote or `manager`-spawned
  drivers, a fresh browser profile per request, the page's real status and
  headers, and any request method. The `View` extractor hands handlers the
  browser to interact with the page.
- `spire-redb`: `RedbStore` keeps a frontier and seen set in redb, so a crawl
  survives a crash and resumes where it stopped.
- Examples for HTML extraction, JSON APIs and browser automation.

## [0.1.0] - 2025-12-01

### Added

- Initial placeholder release for name reservation

[Unreleased]: https://github.com/spire-rs/spire/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/spire-rs/spire/releases/tag/v0.2.0
[0.1.0]: https://crates.io/crates/spire/0.1.0
