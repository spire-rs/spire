# Installation

Spire needs Rust 1.95 or later. Add it with the features you need, and
tokio to run it:

```toml
[dependencies]
spire = { version = "0.3", features = ["reqwest", "macros"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Feature flags

| Feature | Enables |
|---|---|
| `html` (default) | The `Html` and `Element` extractors and the `Select` trait |
| `macros` | `#[derive(Select)]` and `#[derive(Tags)]` |
| `reqwest` | `HttpClient`, the HTTP backend |
| `thirtyfour` | `BrowserBackend`, the WebDriver backend |
| `robots` | Obeying robots.txt with `Crawler::with_robots` |
| `sitemap` | The `Sitemap` extractor and the `FollowSitemap` handler |
| `serde` | Serializable seeds for snapshots, and `JsonlDataset` |
| `redb` | `RedbStore`, a crash-safe frontier and seen set on disk |
| `rustls-tls` (default), `native-tls` | TLS for the enabled backends |
| `testing` | `MockSite` and `Visit`, to test handlers without a network |
| `full` | Everything except `native-tls` and `testing` |

Enable `testing` in your dev-dependencies only; see [Testing](testing.md).

## The HTTP backend's own features

`spire-reqwest` has features of its own for compression, cookies and more.
Depend on it next to `spire` to turn them on:

```toml
[dependencies]
spire-reqwest = { version = "0.3", features = ["gzip", "brotli", "cookies"] }
```

| Feature | Enables |
|---|---|
| `http2` (default) | HTTP/2 negotiation |
| `system-proxy` (default) | Proxies from the environment and system settings |
| `gzip`, `brotli`, `zstd`, `deflate` | Transparent response decompression |
| `cookies` | A cookie store shared by the whole crawl |
| `socks` | SOCKS5 proxies |
| `hickory-dns` | DNS through hickory instead of the system resolver |
