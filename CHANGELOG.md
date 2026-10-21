# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `View` extractor now wraps the browser `WebDriver` (`spire::extract::driver::View`).
- `spire-core` frontier: `Seed`, `InMemFrontier` and the `Dedup`, `MaxDepth`,
  `Retry` and `TagBackoff` facades, chained with `FrontierExt`.
- `Crawler::run` returns `Stats`; `Outcome::Backoff` pauses a tag (e.g. on 429)
  and retries the request.
- `spire_core::service` adapters (`ServiceBackend`, `ServiceClient`,
  `ServiceWorker`) for tower services; `HttpClient` is a `tower::Service`.
- `spire-reqwest`: `StatusPolicy` turns error statuses into errors (`5xx`, `408`
  and `429` retried, `Retry-After` honoured, other `4xx` rejected); `http2`,
  `charset`, `system-proxy`, compression, `cookies`, `socks` and `hickory-dns`
  features.
- `spire-thirtyfour`: `managed()` endpoints behind the `manager` feature,
  round-robin across endpoints, session reset between requests,
  `max_session_age`, and `BrowserBackend::close`.
- `ResponseExt::final_uri` records the URL a response was served from.
- `Queue::follow` resolves relative links against the page's final URL, drops
  fragments and skips non-`http(s)` links; `Link` sets a tag on a link;
  `Queue::follow_all`.
- `spire`: `Html` is a parsed document (scraper) with `title`, `links`,
  `select_one` and `select_all`; `#[derive(Select)]` extracts structs with CSS
  selectors through the `Element` and `Elements` extractors.
- `spire`: `Text` decodes by the response charset; `Attempt` extractor;
  `Router::layer` accepts fallible tower middleware such as timeouts.
- Crawl scope (`FrontierExt::filter`, `allow_hosts`), budgets
  (`Crawler::with_max_requests`, `with_time_limit`), per-request deadlines
  (`with_request_timeout`) and a shutdown grace period.
- Per-host politeness (`with_host_concurrency`, `with_host_delay`).
- robots.txt per RFC 9309 (`robots` feature, robotxt): `with_robots` takes a
  user agent or a `Robots`, which can fetch through a separate backend
  (`with_backend`, e.g. HTTP for a browser crawl), refreshes rules after
  `with_max_age` (24 hours), and retries an unreachable robots.txt
  (`5xx`, `429`, network errors) while disallowing the origin, skipping it
  after `with_unreachable_retries`. Rules are kept per origin, and
  `Crawl-delay` is honoured.
- `Outcome::Abort` skips the rest of a tag; `Crawler::stats` is live.
- URL normalization for dedup (`Seed::canonical_uri`): sorted query, no
  tracking parameters, normalized escapes.
- `InMemFrontier::priority` and `Seed::with_priority`; retry jitter;
  `TagBackoff::with_max_parked`.
- Snapshots: `Frontier::snapshot`, `Crawler::snapshot`, `InMemSeen::snapshot`,
  and serializable `Seed`/`Fingerprint` with the `serde` feature, which also
  adds `JsonlDataset`.
- `Error::uri` records the failed request.
- `BloomSeen`: a fixed-memory `SeenSet` (about 1.2 bytes per URL at 1%) that
  never crawls a duplicate but may skip a new URL; snapshots with
  `BloomSnapshot`.
- `spire-redb`: `RedbStore`, a redb database holding a `RedbFrontier` and an
  exact on-disk `RedbSeen`, so a crawl survives a crash; seeds in flight are
  queued again on reopening. Commits sync once a second by default (a crash
  repeats work, never loses it), or on every commit with `sync_always`.

### Changed

- **Breaking:** TLS feature `rustls-tls` renamed to `rustls` on `spire`, `spire-reqwest` and
  `spire-thirtyfour`, matching upstream `reqwest` 0.13 and `thirtyfour` 0.38.
- Updated `reqwest` to 0.13, `thirtyfour` to 0.38, `deadpool` to 0.13 and `syn` to 3.0.
- MSRV raised to 1.95.
- **Breaking:** `spire-core` redesigned. `Client` (the runner) is now `Crawler`;
  `FlowControl` is now `Outcome`; `RequestQueue` is now `frontier::Queue` with
  `push` and `follow`; `Backend`, `Client`, `Worker`, `Dataset` and `Frontier`
  are native async traits instead of `async_trait`.
- **Breaking:** datasets must be registered on the `Crawler`; asking for an
  unregistered type is an error instead of a silent in-memory default.
- **Breaking:** `HttpClient` wraps a `reqwest::Client` only; use
  `spire_core::service::ServiceClient` to compose tower middleware.
- **Breaking:** `spire-thirtyfour` redesigned: `BrowserBuilder::remote`
  replaces `with_unmanaged`, `BrowserSession` replaces `BrowserConnection`,
  capabilities are thirtyfour's own (headless Chrome by default), and
  WebDriver failures are `spire_core::Error`s.
- **Breaking:** `Frontier::report` returns an `Ack`, so retries refused by the
  frontier count as failures.
- **Breaking:** `Report::Skipped` and `Report::Cancelled`, `Admission::OutOfScope`
  and `Stats::{skipped, cancelled}`; a failing frontier now drains in-flight
  requests before `Crawler::run` returns its error.
- **Breaking:** `spire` redesigned. Router state is given up front with
  `Router::with_state`; extractors and handlers are native async traits;
  `extract::Body` is now `Bytes`; requests without a route fail instead of
  being silently dropped.

### Deprecated

- N/A

### Removed

- `TagQuery`, `Task`, `TaskBuilder`, `RequestSource`, `FutureDataset`,
  `MapData`, `MapErr`, `DataSink`, `DataStream`, the `Metric`/`Trace` backend
  wrappers and the `tracing`/`metric` features of `spire-core` (tracing is
  always on).
- `BrowserError`, `NavigationErrorType`, `BrowserConfig` and
  `BrowserBehaviorConfig`, most of whose settings had no effect.
- `spire`: the `tracing` feature (tracing is always on), the empty
  `middleware` module, the `HttpBackend` extractor, `AttrTag`/`AttrData`, and
  `async-trait`.

### Fixed

- `spire-core` declared no `tokio` features and failed to build outside the workspace.
- `spire-reqwest` dropped request and response bodies; both are now streamed through.
- The crawler ran requests one at a time regardless of the concurrency limit.
- Wait/hold (now backoff) and abort flow control had no effect.
- `#[derive(Select)]` and `Elements` were unimplemented (`todo!()`).
- `Router::layer` did not wrap the fallback.
- `spire-thirtyfour` ignored managed endpoints, behavior settings and several
  pool options, always used one endpoint, and spoofed the user agent after
  navigating.

### Security

- N/A

## [0.2.0] - 2025-12-01

### Added

- Initial release of the Spire web scraping framework
- Flexible architecture built on tokio and tower
- Multiple backend support (reqwest for HTTP, thirtyfour for browser automation)
- Type-safe routing with tag-based request handling
- Ergonomic extractors for data extraction from requests
- Built-in middleware support via tower integration
- Graceful shutdown with cancellation token support
- Dataset operations for managing scraped data
- Comprehensive error handling with `ErrorKind` enum
- Optional tracing support for observability
- Feature flags for modular functionality
- Comprehensive documentation and examples

## [0.1.0] - 2025-12-01

### Added

- Initial placeholder release for name reservation

[Unreleased]: https://github.com/spire-rs/spire/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/spire-rs/spire/releases/tag/v0.2.0
[0.1.0]: https://github.com/spire-rs/spire/releases/tag/v0.1.0
