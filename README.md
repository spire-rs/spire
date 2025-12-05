# Spire

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]
[![Crate Coverage][coverage-badge]][coverage-url]

**Check out other `spire` projects [here](https://github.com/spire-rs).**

[action-badge]: https://img.shields.io/github/actions/workflow/status/spire-rs/spire/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/spire-rs/spire/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/spire.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/spire
[docs-badge]: https://img.shields.io/docsrs/spire?logo=Docs.rs&style=flat-square
[docs-url]: http://docs.rs/spire
[coverage-badge]: https://img.shields.io/codecov/c/github/spire-rs/spire?logo=codecov&logoColor=white&style=flat-square
[coverage-url]: https://app.codecov.io/gh/spire-rs/spire

The flexible crawler & scraper framework powered by [tokio][tokio-rs/tokio] and
[tower][tower-rs/tower].

[tokio-rs/tokio]: https://github.com/tokio-rs/tokio/
[tower-rs/tower]: https://github.com/tower-rs/tower/

## Overview

Spire is a modular web scraping and crawling framework for Rust that combines
the power of async/await with the composability of tower's middleware ecosystem.
It supports both HTTP-based scraping and browser automation through pluggable
backends.

## Features

- **Multiple Backends**: HTTP (reqwest) and browser automation (thirtyfour)
  support
- **Tower Integration**: Composable middleware using the tower ecosystem
- **Type-Safe Routing**: Tag-based routing with compile-time guarantees
- **Ergonomic Extractors**: Clean, type-safe data extraction from requests
- **Async/Await**: Built on tokio for high-performance concurrent scraping
- **Observability**: Optional tracing and metrics support
- **Graceful Shutdown**: Proper resource cleanup and cancellation support

## Quick Start

Add spire to your `Cargo.toml`:

```toml
[dependencies]
spire = { version = "0.2.0", features = ["reqwest", "tracing"] }
tracing = { version = "0.1", features = [] }
tracing-subscriber = { version = "0.3", features = [] }
tokio = { version = "1.0", features = ["rt-multi-thread", "macros"] }
```

Basic HTTP scraping example:

```rust
use spire::prelude::*;

async fn scrape_page(
    uri: http::Uri,
    data_store: Data<String>,
    Text(html): Text,
) -> Result<()> {
    let url = uri.to_string();
    tracing::info!("Scraped {}: {} bytes", url, html.len());

    // Store the scraped data
    data_store.write(format!("Content from {}", url)).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize structured logging
    tracing_subscriber::fmt::init();

    let router = Router::new()
        .route("page", scrape_page);

    let client = Client::new(HttpClient::default(), router)
        .with_request_queue(InMemDataset::stack())
        .with_dataset(InMemDataset::<String>::new());

    client.request_queue()
        .append_with_tag("page", "https://example.com")
        .await?;

    client.run().await
}
```

See the [examples directory](./examples/) for comprehensive guides and the
[main crate documentation](./spire/) for detailed API reference. usage.

## Contributing

We welcome contributions! Please read our
[Contributing Guide](./CONTRIBUTING.md) for details.

## License

This project is licensed under the MIT License - see the [LICENSE](./LICENSE)
file for details.
