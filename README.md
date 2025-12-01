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

Spire is a modular web scraping and crawling framework for Rust that combines the power of async/await with the composability of tower's middleware ecosystem. It supports both HTTP-based scraping and browser automation through pluggable backends.

## Features

- **Multiple Backends**: HTTP (reqwest) and browser automation (thirtyfour) support
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
spire = { version = "0.2.0", features = ["reqwest"] }
```

Basic HTTP scraping example:

```rust
use spire::prelude::*;
use spire::extract::Text;
use spire::context::{RequestQueue, Tag};
use spire::reqwest_backend::HttpClient;
use spire::dataset::InMemDataset;

async fn handler(Text(html): Text) -> Result<(), Box<dyn std::error::Error>> {
    println!("Scraped {} bytes", html.len());
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let router = Router::new()
        .route(Tag::new("main"), handler);

    let backend = HttpClient::default();
    let client = Client::new(backend, router)
        .with_request_queue(InMemDataset::stack())
        .with_dataset(InMemDataset::<String>::new());

    client.queue()
        .push(Tag::new("main"), "https://example.com")
        .await?;

    client.run().await?;
    Ok(())
}
```

See the [main crate documentation](./spire/) for more examples and detailed usage.

## Contributing

We welcome contributions! Please read our [Contributing Guide](./CONTRIBUTING.md) for details.

## License

This project is licensed under the MIT License - see the [LICENSE](./LICENSE) file for details.
