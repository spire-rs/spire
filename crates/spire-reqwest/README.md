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

`HttpClient` implements both the `Backend` and `Client` traits from
`spire-core`: every request of a crawl shares one connection pool.

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
spire = { version = "0.4.0", features = ["reqwest"] }
```

### Features

TLS through rustls, HTTP/2 and system proxies are on by default. Everything else is opt-in:

```toml
[dependencies]
spire-reqwest = { version = "0.4.0", features = ["gzip", "brotli", "cookies"] }

# Native TLS instead of rustls
spire-reqwest = { version = "0.4.0", default-features = false, features = ["native-tls", "http2"] }
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

let backend = HttpClient::from(reqwest_client);
```

## Error Handling

Failed transfers are classified so the crawler retries only what may succeed
next time: timeouts and connection errors are retried, invalid requests and
redirect loops are not.

Error statuses follow the client's `StatusPolicy`. By default, `408`, `429`
and `5xx` become retryable errors, with `429` and `503` pausing the request's
host for as long as `Retry-After` asks, and other `4xx` become errors that are
not retried. `StatusPolicy::Accept` returns every response to the handler
instead:

```rust
use spire_reqwest::{HttpClient, StatusPolicy};

let backend = HttpClient::new().with_status_policy(StatusPolicy::Accept);
```

Responses record the URL they were served from after redirects, available
through `spire_core::request::ResponseExt::final_uri`.

## Performance Considerations

- The reqwest client maintains connection pools automatically.
- Cookies and session state are maintained per reqwest client instance.
- Consider reusing `HttpClient` instances when possible to benefit from
  connection pooling.

## Contributing

This crate follows the same contribution guidelines as the main spire project.
Please see [CONTRIBUTING.md](../../CONTRIBUTING.md) for details.

## License

This project is licensed under the MIT License; see the
[LICENSE](../../LICENSE) file for details.
