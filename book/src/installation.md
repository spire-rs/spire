# Installation

Spire needs Rust 1.95 or later. Add it with the features you need, and
tokio to run it:

```toml
[dependencies]
spire = { version = "0.9", features = ["reqwest", "macros"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

The default features provide HTML parsing and Rustls support for enabled
backends. `reqwest` adds the HTTP backend, and `macros` adds derives for
HTML extraction, tags and state. No backend is enabled by default.

For browser automation, use `thirtyfour` instead of `reqwest`; see
[Browsers](browsers.md) for WebDriver setup. Optional capabilities such as
model extraction, persistent storage and robots.txt support can be added
as needed. The [feature reference](features.md) lists all flags, including
backend-specific options.

Continue with [Your first crawl](first-crawl.md) to fetch pages, follow
links and store typed results.
