# Feature flags

Choose features for the parts of Spire your application uses. For a usual
HTTP crawl, start with the dependency setup in [Installation](installation.md).

## Spire facade

| Feature | Enables |
|---|---|
| `html` (default) | HTML extractors including `Html` and `Links`, the `Select` trait, `Collect` and `FollowLinks` |
| `agent` | Model extraction through `Extract<T>`; also enables `markdown` |
| `markdown` | The `Markdown` extractor; also enables `html` |
| `macros` | `Select`, `FromRef`, `Tags` and `Revisit` derives; also enables `html` |
| `reqwest` | `ReqwestBackend`, the HTTP backend |
| `thirtyfour` | `ThirtyfourBackend`, the WebDriver backend |
| `user-agents` | Browser user agents with matching navigation headers for identities |
| `robots` | Obeying robots.txt with `Crawler::with_robots` |
| `sitemap` | The `Sitemap` extractor and the `FollowSitemap` handler |
| `serde` | Serializable seeds for snapshots |
| `json` | `JsonlDataset` |
| `csv` | `CsvDataset` |
| `redb` | `RedbStore`, a crash-safe frontier, seen set and datasets on disk; persistent answer caching with `agent` |
| `rustls-tls` (default), `native-tls` | TLS for the enabled backends |
| `testing` | `MockSite`, `MockPage`, `MockSession` and `Visit`, for offline tests |
| `full` | Everything except `native-tls` and `testing` |

No backend is enabled by default. Enable `testing` in your dev-dependencies only; see [Testing handlers](testing.md).

`full` enables all the facade features above except `native-tls` and
`testing`, including persistent answer caching. Rig integration and browser
driver management have their own flags below.

## HTTP backend

`spire-reqwest` has features of its own for compression, cookies and more.
Depend on it next to `spire` to turn them on:

```toml
[dependencies]
spire-reqwest = { version = "0.8", features = ["gzip", "brotli", "cookies"] }
```

| Feature | Enables |
|---|---|
| `http2` (default) | HTTP/2 negotiation |
| `system-proxy` (default) | Proxies from the environment and system settings |
| `gzip`, `brotli`, `zstd`, `deflate` | Transparent response decompression |
| `cookies` | A cookie store shared by the whole crawl |
| `socks` | SOCKS5 proxies |
| `hickory-dns` | DNS through hickory instead of the system resolver |

## Browser backend

`spire-thirtyfour` enables `rustls-tls` by default and offers `native-tls`
as an alternative. Both concern the connection to the WebDriver server,
not the browser's own TLS stack. Enable `manager` on that crate to download,
cache and start a driver; the browser itself must already be installed.
`manager` is not enabled by `spire`'s `full` feature.

```toml
[dependencies]
spire = { version = "0.8", features = ["thirtyfour"] }
spire-thirtyfour = { version = "0.8", features = ["manager"] }
```

| Feature | Enables |
|---|---|
| `rustls-tls` (default) | Rustls TLS to the WebDriver server |
| `native-tls` | Platform-native TLS to the WebDriver server |
| `manager` | Automatic driver management through `ThirtyfourBuilder::managed` |

## Using spire-core directly

Backend, frontier and dataset implementations can depend on `spire-core`
without the router and extractors in `spire`. Its feature flags are:

No features are enabled by default.

| Feature | Enables |
|---|---|
| `robots` | Fetching and obeying robots.txt with `Crawler::with_robots` |
| `serde` | Serialization of `Seed`, `Fingerprint` and `BloomSnapshot` |
| `json` | `JsonlDataset`, writing JSON Lines; also enables `serde` |
| `csv` | `CsvDataset`, writing CSV; also enables `serde` |
| `testing` | `MockSite`, `MockPage`, `MockSession` and `Visit` for offline tests |
| `user-agents` | `Identity::with_profile` and the uagent `Profile`, `UserAgents`, `Browser` and `Platform` re-exports |

## Model extraction

Enable `agent` for model extraction. To use Rig models, also enable `rig`
on `spire-agent`:

```toml
[dependencies]
spire = { version = "0.8", features = ["agent", "redb", "reqwest"] }
spire-agent = { version = "0.8", features = ["rig"] }
```

The facade's `redb` feature also enables persistent answer caching when
`agent` is enabled. It does not enable model extraction on its own.
`full` includes `agent` and `redb`; enable Rig separately as above.
See [Model extraction](model-extraction.md) for provider configuration and
cache usage.
