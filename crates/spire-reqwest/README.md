# spire-reqwest

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]

**Check out other `spire` projects [here](https://github.com/spire-rs).**

[action-badge]: https://img.shields.io/github/actions/workflow/status/spire-rs/spire/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/spire-rs/spire/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/spire-reqwest.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/spire-reqwest
[docs-badge]: https://img.shields.io/docsrs/spire-reqwest?logo=Docs.rs&style=flat-square
[docs-url]: http://docs.rs/spire-reqwest

HTTP backend for Spire, built on
[reqwest](https://github.com/seanmonstar/reqwest).

## Overview

`ReqwestBackend` implements `Backend`, and `ReqwestSession` implements
`Session`, from `spire-core`: requests share the backend's connection pool.

- **Configurable client**: the backend builds its reqwest client from a
  function you pass, with your timeouts, user agent and other options.
- **Identities**: with `Crawler::with_identities`, each identity's requests go
  through a client of its own, with its proxy and, with the `cookies` feature,
  its own cookie store. SOCKS proxies need the `socks` feature.
- **Errors and statuses**: failures are classified so only what may succeed is
  retried, and error statuses follow a `StatusPolicy`.

## Installation

Enable the `reqwest` feature of `spire`, for `spire::backend::ReqwestBackend`:

```toml
[dependencies]
spire = { version = "0.11", features = ["reqwest"] }
```

Or depend on this crate and `spire-core` directly, as the examples below do:

```toml
[dependencies]
spire-core = { version = "0.11" }
spire-reqwest = { version = "0.11", features = ["gzip", "cookies"] }
```

`rustls-tls`, `http2` and `system-proxy` are enabled by default:

| Feature | Enables |
|---|---|
| `rustls-tls` | TLS through rustls, with no system dependencies |
| `native-tls` | TLS through the platform's native library |
| `http2` | HTTP/2 negotiation over TLS |
| `system-proxy` | Proxies from the environment and system settings |
| `gzip`, `brotli`, `zstd`, `deflate` | Transparent response decompression |
| `cookies` | A cookie store for direct requests and a separate one per identity |
| `socks` | SOCKS5 proxies |
| `hickory-dns` | DNS resolution through hickory instead of the system resolver |

## Usage

`ReqwestBackend::from_builder` takes a function that makes the reqwest
configuration, which a crawl with identities builds one client per identity
from:

```rust
use std::time::Duration;

use spire_reqwest::{ReqwestBackend, reqwest};

let backend = ReqwestBackend::from_builder(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("MyBot/1.0")
})?;
# Ok::<_, spire_core::Error>(())
```

## Errors and statuses

Failed transfers are classified so the crawler retries only what may succeed
next time: timeouts and connection errors are retried, invalid requests and
redirect loops are not.

Error statuses follow the client's `StatusPolicy`. By default, `408`, `429`
and `5xx` become retryable errors, with a valid `Retry-After` pausing the request's
host for as long as `Retry-After` asks, and other `4xx` become errors that are
not retried. `StatusPolicy::Accept` returns every response to the handler
instead:

```rust
use spire_core::backend::StatusPolicy;
use spire_reqwest::ReqwestBackend;

let backend = ReqwestBackend::new().with_status_policy(StatusPolicy::Accept);
```

Responses record the URL they were served from after redirects, available
through `spire_core::http::ResponseExt::final_uri`.

## Learn more

- The book's [HTTP](https://spire-rs.github.io/spire/http.html)
  and [Identities](https://spire-rs.github.io/spire/identities.html) pages,
  and [Feature flags](https://spire-rs.github.io/spire/features.html#http-backend)
- The [API documentation](https://docs.rs/spire-reqwest)

## Contributing

See [CONTRIBUTING.md](https://github.com/spire-rs/spire/blob/main/CONTRIBUTING.md)
for development setup and guidelines.

## License

MIT License, see [LICENSE](https://github.com/spire-rs/spire/blob/main/LICENSE).
