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

This crate provides [`HttpClient`], an HTTP backend implementation that
integrates with the Spire web scraping framework using the popular
[reqwest](https://github.com/seanmonstar/reqwest) library.

## Overview

`spire-reqwest` provides an HTTP client backend implementation for Spire on top
of the popular [reqwest](https://github.com/seanmonstar/reqwest) library. This
backend enables high-performance HTTP-based web scraping with support for
cookies, headers, redirects, and other HTTP features.

The backend implements the `Backend` trait from `spire-core` and creates
`HttpConnection` instances that implement the `Client` trait for performing HTTP
requests.

## Features

- **Built on Reqwest**: Leverages the popular reqwest HTTP client library
- **Spire Integration**: Seamlessly integrates with the Spire web scraping
  framework
- **Async/Await**: Fully async implementation for better performance
- **TLS Support**: Choose between rustls (default) and native TLS
  implementations

## Usage

This crate is typically not used directly. Instead, enable the `reqwest` feature
in the main `spire` crate and refer to the
[spire documentation](https://docs.rs/spire) for usage examples and guides.

```toml
[dependencies]
spire = { version = "0.2.0", features = ["reqwest"] }
```

### TLS Configuration

By default, `spire-reqwest` uses rustls for TLS connections. You can choose
between different TLS implementations:

```toml
# Default: uses rustls-tls
[dependencies]
spire-reqwest = "0.2.0"

# Explicitly use rustls
[dependencies]
spire-reqwest = { version = "0.2.0", features = ["rustls-tls"], default-features = false }

# Use native TLS (system TLS library)
[dependencies]
spire-reqwest = { version = "0.2.0", features = ["native-tls"], default-features = false }
```

For advanced usage and custom configurations, see the
[API documentation](https://docs.rs/spire-reqwest).

## Advanced Usage Examples

### Creating from reqwest Client

```rust
use spire_reqwest::HttpClient;
use std::time::Duration;

let reqwest_client = reqwest::Client::builder()
    .timeout(Duration::from_secs(30))
    .user_agent("MyBot/1.0")
    .build()
    .unwrap();

let backend = HttpClient::from_client(reqwest_client);
```

### Creating from Tower Service

```rust
use spire_reqwest::{HttpClient, HttpService, client_to_service};

let reqwest_client = reqwest::Client::new();
let service: HttpService = client_to_service(reqwest_client);
let backend = HttpClient::from_service(service);
```

## Error Handling

All reqwest errors are automatically converted to `spire_core::Error` types.
This includes network errors, HTTP errors, and parsing errors.

## Performance Considerations

- The reqwest client maintains connection pools automatically.
- Multiple `HttpConnection` instances can share the same underlying reqwest
  client.
- Cookies and session state are maintained per reqwest client instance.
- Consider reusing `HttpClient` instances when possible to benefit from
  connection pooling.

## Contributing

This crate follows the same contribution guidelines as the main spire project.
Please see [CONTRIBUTING.md](../../CONTRIBUTING.md) for details.

## License

This project is licensed under the MIT License - see the
[LICENSE.txt](../../LICENSE.txt) file for details.
