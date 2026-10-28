# Feature flags

Choose features for the parts of Spire your application uses. For a usual
HTTP crawl, start with the dependency setup in [Installation](installation.md).

## Spire facade

| Feature | Enables |
|---|---|
| `html` (default) | HTML extractors including `Html` and `Links`, the `Select` trait, `Collect` and `FollowLinks` |
| `agent` | Model extraction; also enables `markdown` |
| `agent-rig` | Rig model adapter and the upstream Rig crate; enables `agent` |
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
| `testing` | `MockSite`, `MockPage`, `MockSession` and `Visit`; also `testing::Script` with `agent` |
| `full` | Everything except `native-tls` and `testing` |

No backend is enabled by default. Enable `testing` in your dev-dependencies only; see [Testing handlers](testing.md).

`full` enables all the facade features above except `native-tls` and
`testing`, including Rig integration and persistent answer caching. Browser driver
management has its own flag below.

`spire-macros` and `spire-redb` have no feature flags of their own. Enable
`macros` or `redb` on the facade to use their re-exports, or depend on those
crates directly.

## HTTP backend

`spire-reqwest` has features of its own for compression, cookies and more.
Depend on it next to `spire` to turn them on:

```toml
[dependencies]
spire-reqwest = { version = "0.8", features = ["gzip", "brotli", "cookies"] }
```

| Feature | Enables |
|---|---|
| `rustls-tls` (default), `native-tls` | TLS implementation |
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

## AI-driven crawling

`agent` enables model extraction and [objective middleware](objectives.md). Provider integrations are
configured on `spire-agent`; see [Model providers](agent-providers.md).
The facade's `redb` feature also enables answer caching when `agent` is
present. See [Model budgets and caches](agent-resources.md) for setup.

No features are enabled by default on `spire-agent`:

| Feature | Enables |
|---|---|
| `rig` | The `Rig` adapter and upstream Rig crate re-export |
| `redb` | `RedbCache` and the `RedbStore` implementation of `AgentStoreExt` |
| `testing` | `testing::Script`, an offline provider |

Enable `agent-rig` on the facade or `rig` directly on `spire-agent` for Rig
integration. The facade's `agent + redb` features enable persistent answer
caching together, and `agent + testing` enable `spire::testing::Script`.
