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

Reqwest-based HTTP client backend for Spire.

This crate provides [`HttpClient`], an HTTP backend implementation that integrates with the Spire web scraping framework using the popular [reqwest](https://github.com/seanmonstar/reqwest) library.

## Overview

`spire-reqwest` provides an HTTP client backend implementation for spire using the reqwest library. This backend enables high-performance HTTP-based web scraping with support for cookies, headers, redirects, and other HTTP features.

The backend implements the `Backend` trait from `spire-core` and creates `HttpConnection` instances that implement the `Client` trait for performing HTTP requests.

## Key Features

- **High Performance**: Built on reqwest and hyper for efficient HTTP operations
- **Cookie Support**: Automatic cookie jar management for session handling
- **Flexible Configuration**: Customizable timeouts, headers, and client settings
- **Redirect Handling**: Configurable redirect following
- **Proxy Support**: HTTP and SOCKS proxy support via reqwest
- **TLS/SSL**: Full TLS support with certificate validation options
- **Connection Pooling**: Efficient connection reuse through reqwest's connection pooling
- **Async/Await**: Fully async implementation for better performance

## Usage

### Basic Usage with Spire

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
use spire_reqwest::HttpClient;
use spire_core::dataset::InMemDataset;

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
    
    let client = spire_core::Client::new(backend, router)
        .with_request_queue(InMemDataset::stack())
        .with_dataset(InMemDataset::<String>::new());

    client.queue()
        .push(Tag::new("main"), "https://example.com")
        .await?;

    client.run().await?;
    Ok(())
}
```

### Custom Configuration

You can customize the HTTP client backend with reqwest client configuration:

```rust
use spire_reqwest::HttpClient;
use spire_core::backend::Backend;
use reqwest::Client;
use std::time::Duration;

// Create a custom reqwest client
let reqwest_client = Client::builder()
    .timeout(Duration::from_secs(30))
    .user_agent("MyBot/1.0")
    .cookie_store(true)
    .danger_accept_invalid_certs(false)
    .redirect(reqwest::redirect::Policy::limited(10))
    .build()?;

// Use it with spire
let backend = HttpClient::from_client(reqwest_client);

// Connect to get a client instance
let connection = backend.connect().await?;
```

### Direct Usage

If you need to use this crate directly (for custom backend implementations):

```toml
[dependencies]
spire-reqwest = "0.2.0"
spire-core = "0.2.0"
```

```rust
use spire_reqwest::HttpClient;
use spire_core::backend::{Backend, Client};
use spire_core::context::Request;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create backend
    let backend = HttpClient::default();
    
    // Connect to get a client
    let connection = backend.connect().await?;
    
    // Build a request
    let request = Request::builder()
        .method("GET")
        .uri("https://httpbin.org/get")
        .body(Default::default())?;
    
    // Make the request
    let response = connection.resolve(request).await?;
    
    println!("Status: {}", response.status());
    Ok(())
}
```

## Advanced Configuration

### Proxy Support

```rust
use reqwest::{Client, Proxy};
use spire_reqwest::HttpClient;

let proxy = Proxy::http("http://proxy.example.com:8080")?;
let reqwest_client = Client::builder()
    .proxy(proxy)
    .build()?;
    
let backend = HttpClient::from_client(reqwest_client);
```

### Custom Headers and User Agents

```rust
use reqwest::{Client, header::{HeaderMap, HeaderValue, USER_AGENT}};
use spire_reqwest::HttpClient;

let mut headers = HeaderMap::new();
headers.insert(USER_AGENT, HeaderValue::from_static("MyBot/1.0"));
headers.insert("X-Custom-Header", HeaderValue::from_static("custom-value"));

let reqwest_client = Client::builder()
    .default_headers(headers)
    .build()?;
    
let backend = HttpClient::from_client(reqwest_client);
```

### Timeout Configuration

```rust
use reqwest::Client;
use spire_reqwest::HttpClient;
use std::time::Duration;

let reqwest_client = Client::builder()
    .timeout(Duration::from_secs(30))
    .connect_timeout(Duration::from_secs(10))
    .build()?;
    
let backend = HttpClient::from_client(reqwest_client);
```

## Architecture

This crate provides two main types:

- **`HttpClient`**: Implements the `Backend` trait and manages reqwest client configuration
- **`HttpConnection`**: Implements the `Client` trait and performs individual HTTP requests

The backend uses internal utility functions to convert between spire's HTTP types and reqwest's types:

- `request_to_reqwest()`: Converts `spire_core::context::Request` to `reqwest::Request`
- `response_from_reqwest()`: Converts `reqwest::Response` to `spire_core::context::Response`

## Error Handling

All reqwest errors are automatically converted to `spire_core::Error` types. This includes:

- Network errors (connection failures, timeouts)
- HTTP errors (4xx, 5xx status codes)
- Parsing errors (invalid URLs, malformed headers)

## Performance Considerations

- The reqwest client maintains connection pools automatically
- Multiple `HttpConnection` instances can share the same underlying reqwest client
- Cookies and session state are maintained per reqwest client instance
- Consider reusing `HttpClient` instances when possible to benefit from connection pooling

## Limitations

- Request and response body handling is currently simplified (TODO items in the code)
- Some advanced reqwest features may not be exposed through the spire interface
- HTTP/2 and HTTP/3 support depends on reqwest configuration

## Contributing

This crate follows the same contribution guidelines as the main spire project. Please see [CONTRIBUTING.md](../../CONTRIBUTING.md) for details.

## License

This project is licensed under the MIT License - see the [LICENSE.txt](../../LICENSE.txt) file for details.