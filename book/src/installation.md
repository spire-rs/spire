# Installation

Spire needs Rust 1.95 or later, and runs on tokio. Add it with the features
you need, and tokio to run it:

```toml
[dependencies]
spire = { version = "0.11", features = ["reqwest", "macros"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

The default features are `html`, for parsing pages, and `rustls-tls`, for TLS
in whichever backend is enabled. No backend is enabled by default: `reqwest`
adds the HTTP backend, and `macros` adds the derives for HTML extraction,
tags, revisits and state.

## Common setups

Add features as a crawl needs them:

| To | Add | See |
|---|---|---|
| Crawl over HTTP | `reqwest` | [HTTP](http.md) |
| Crawl in a real browser | `thirtyfour`, and a WebDriver server such as chromedriver | [Browsers](browsers.md) |
| Obey robots.txt | `robots` | [Limits and politeness](politeness.md#robotstxt) |
| Follow sitemaps | `sitemap` | [Sitemaps](sitemaps.md) |
| Survive a crash and resume | `redb` or `sqlite` | [Persistence with redb](redb.md), [Persistence with SQLite](sqlite.md) |
| Write results to files | `json` or `csv` | [Storing results](datasets.md) |
| Extract data with a model | `agent`, and `rig` for Rig's providers | [Model extraction](model-extraction.md) |

`full` enables all of them but `native-tls` and `testing`.

To test handlers against fixed pages, without a network, enable `testing` in
your dev-dependencies only:

```toml
[dev-dependencies]
spire = { version = "0.11", features = ["testing"] }
```

The [feature reference](features.md) lists every flag, including those of the
backends. Continue with [Your first crawl](first-crawl.md) to fetch pages,
follow links and store typed results.
