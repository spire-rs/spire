# spire-reqwest

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]
[![Crate Coverage][coverage-badge]][coverage-url]

**Check out other `spire` projects [here](https://github.com/spire-rs).**

[action-badge]: https://img.shields.io/github/actions/workflow/status/spire-rs/spire/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/spire-rs/spire/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/spire-reqwest.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/spire-reqwest
[docs-badge]: https://img.shields.io/docsrs/spire-reqwest?logo=Docs.rs&style=flat-square
[docs-url]: http://docs.rs/spire-reqwest
[coverage-badge]: https://img.shields.io/codecov/c/github/spire-rs/spire?logo=codecov&logoColor=white&style=flat-square
[coverage-url]: https://app.codecov.io/gh/spire-rs/spire

HTTP client backend for the spire web scraping framework, powered by [reqwest](https://github.com/seanmonstar/reqwest).

## Overview

`spire-reqwest` provides an HTTP client backend implementation for spire using the popular reqwest library. This backend enables high-performance HTTP-based web scraping with support for cookies, headers, redirects, and other HTTP features.

## Key Features

- **High Performance**: Built on reqwest and hyper for efficient HTTP operations
- **Cookie Support**: Automatic cookie jar management for session handling
- **Flexible Configuration**: Customizable timeouts, headers, and client settings
- **Redirect Handling**: Configurable redirect following
- **Proxy Support**: HTTP and SOCKS proxy support via reqwest
- **TLS/SSL**: Full TLS support with certificate validation options

## Usage

This crate is typically not used directly. Instead, enable the `reqwest` feature in the main `spire` crate:

```toml
[dependencies]
spire = { version = "0.2.0", features = ["reqwest"] }
```

Then use the HTTP client backend in your spire applications:

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

    // Create HTTP client backend with default settings
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

## Configuration

You can customize the HTTP client backend:

```rust
use spire::reqwest_backend::HttpClient;
use reqwest::Client;
use std::time::Duration;

// Create a custom reqwest client
let reqwest_client = Client::builder()
    .timeout(Duration::from_secs(30))
    .user_agent("MyBot/1.0")
    .cookie_store(true)
    .build()?;

// Use it with spire
let backend = HttpClient::new(reqwest_client);
```

## Direct Usage

If you need to use this crate directly (for custom backend implementations):

```toml
[dependencies]
spire-reqwest = "0.2.0"
```

```rust
use spire_reqwest::HttpClient;
use spire_core::backend::Backend;

let backend = HttpClient::default();
let client = backend.connect()?;
```

## License

This project is licensed under the MIT License - see the [LICENSE](../LICENSE) file for details.