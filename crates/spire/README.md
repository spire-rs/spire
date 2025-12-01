# Spire

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]
[![Crate Coverage][coverage-badge]][coverage-url]

**Check out other `spire` projects [here](https://github.com/spire-rs).**

> [!WARNING]
> Work in progress. The API is not yet stable and may change.

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

## Features

- **Flexible Architecture**: Built on tower's Service trait for composable middleware
- **Multiple Backends**: Support for HTTP (reqwest) and WebDriver (thirtyfour) backends
- **Type-Safe Routing**: Tag-based routing with compile-time safety
- **Async First**: Powered by tokio for high-performance concurrent scraping
- **Ergonomic Extractors**: Extract data from requests with a clean, type-safe API
- **Graceful Shutdown**: Built-in support for clean shutdown and resource cleanup
- **Observability**: Optional tracing support for debugging and monitoring

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
spire = "0.2.0"
```

### Feature Flags

- **`reqwest`** - Enables the reqwest-based HTTP client backend
- **`thirtyfour`** - Enables the WebDriver/browser automation backend
- **`macros`** - Enables procedural macros for deriving extractors
- **`tracing`** - Enables tracing/logging support
- **`trace`** - Enables detailed trace-level instrumentation
- **`metric`** - Enables metrics collection
- **`full`** - Enables all features (macros, tracing, reqwest, thirtyfour)

## Quick Start

### HTTP Scraping with Reqwest

```rust,no_run
use spire::prelude::*;
use spire::extract::{Text, State};
use spire::context::{RequestQueue, Tag};
use spire::reqwest_backend::HttpClient;
use spire::dataset::InMemDataset;

#[derive(Clone)]
struct AppState {
    api_key: String,
}

async fn scrape_handler(
    State(state): State<AppState>,
    Text(html): Text,
    queue: RequestQueue,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Scraped {} bytes with API key: {}", html.len(), state.api_key);
    
    // Queue more requests
    queue.push("page2", "https://example.com/page2").await?;
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create router
    let router = Router::new()
        .route("main", scrape_handler)
        .with_state(AppState {
            api_key: "my-api-key".to_string(),
        });

    // Create backend and client
    let backend = HttpClient::default();
    let client = Client::new(backend, router)
        .with_request_queue(InMemDataset::stack())
        .with_dataset(InMemDataset::<String>::new());

    // Start initial request
    client.queue()
        .push("main", "https://example.com")
        .await?;

    // Run the client
    client.run().await?;

    Ok(())
}
```

### Browser Automation with ThirtyFour

```rust,no_run
use spire::prelude::*;
use spire::extract::State;
use spire::context::{RequestQueue, Tag};
use spire::thirtyfour_backend::BrowserPool;
use spire::dataset::InMemDataset;

async fn browser_handler(
    queue: RequestQueue,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Processing page with browser");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create router
    let router = Router::new()
        .route("main", browser_handler);

    // Create browser pool backend
    let backend = BrowserPool::builder().build();
    
    let client = Client::new(backend, router)
        .with_request_queue(InMemDataset::stack())
        .with_dataset(InMemDataset::<String>::new());

    // Start initial request
    client.queue()
        .push("main", "https://example.com")
        .await?;

    // Run the client
    client.run().await?;

    Ok(())
}
```

## Architecture

Spire is built on several key abstractions:

- **Router**: Routes requests to handlers based on tags
- **Handler**: Async functions that process requests and extract data
- **Extractor**: Type-safe data extraction from request context
- **Backend**: Pluggable backend for HTTP or browser automation
- **Dataset**: Storage for scraped data and request queues

## Middleware

Spire integrates seamlessly with the tower ecosystem:

```rust,no_run
use tower::ServiceBuilder;
use tower::timeout::TimeoutLayer;
use std::time::Duration;
use spire::prelude::*;

async fn handler() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

let router = Router::new()
    .route("main", handler)
    .layer(
        ServiceBuilder::new()
            .timeout(Duration::from_secs(30))
            .into_inner()
    );
```

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](../CONTRIBUTING.md) for guidelines.

## License

This project is licensed under the MIT License - see the [LICENSE](../LICENSE) file for details.

## Acknowledgments

- Built on [tokio][tokio-rs/tokio] for async runtime
- Uses [tower][tower-rs/tower] for middleware composition
- Routing pattern inspired by [axum][tokio-rs/axum]

[tokio-rs/axum]: https://github.com/tokio-rs/axum/
