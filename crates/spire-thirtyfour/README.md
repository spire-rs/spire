# spire-thirtyfour

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]
[![Crate Coverage][coverage-badge]][coverage-url]

**Check out other `spire` projects [here](https://github.com/spire-rs).**

[action-badge]: https://img.shields.io/github/actions/workflow/status/spire-rs/spire/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/spire-rs/spire/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/spire-thirtyfour.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/spire-thirtyfour
[docs-badge]: https://img.shields.io/docsrs/spire-thirtyfour?logo=Docs.rs&style=flat-square
[docs-url]: http://docs.rs/spire-thirtyfour
[coverage-badge]: https://img.shields.io/codecov/c/github/spire-rs/spire?logo=codecov&logoColor=white&style=flat-square
[coverage-url]: https://app.codecov.io/gh/spire-rs/spire

Browser automation backend for Spire, built on
[thirtyfour](https://github.com/stevepryde/thirtyfour) (WebDriver).

## Overview

`BrowserBackend` keeps a pool of WebDriver sessions and hands one to every
request. Fetching a request navigates to it and returns the rendered page
source, so JavaScript-heavy pages scrape like any other. Through `Deref`, the
session also exposes the full `WebDriver` API for clicking, scrolling and
waiting on the page.

- **Endpoints**: remote WebDriver servers (chromedriver, geckodriver, Selenium
  grids), rotated between in turn, or drivers downloaded and spawned
  automatically with the `manager` feature.
- **Pooling**: sessions start on demand up to a limit and are reused; storage
  and cookies are cleared between requests unless you keep them.
- **Errors**: WebDriver failures map to `spire_core::ErrorKind`s, so timeouts
  and navigation failures are retried while page bugs are not.

## Usage

Enable the `thirtyfour` feature of the main `spire` crate, or depend on this
crate directly to use the backend with `spire-core`:

```toml
[dependencies]
spire = { version = "0.2.0", features = ["thirtyfour"] }
```

```rust,no_run
use std::time::Duration;

use spire_thirtyfour::BrowserBackend;

# async fn example() -> spire_core::Result<()> {
let backend = BrowserBackend::builder()
    .remote("http://localhost:4444")
    .max_sessions(4)
    .page_load_timeout(Duration::from_secs(20))
    .build()?;

// Crawl with `spire::Crawler::new(backend.clone(), router)`, then quit the
// sessions; sessions that are dropped instead block the runtime.
backend.close().await;
# Ok(())
# }
```

Sessions start headless Chrome by default. Pass your own capabilities for
another browser or for browser flags:

```rust
use spire_thirtyfour::BrowserBackend;
use spire_thirtyfour::thirtyfour::DesiredCapabilities;

let builder = BrowserBackend::builder()
    .remote("http://localhost:4444")
    .capabilities(DesiredCapabilities::firefox());
```

## Features

- **`rustls`** (default): TLS to the WebDriver server through rustls.
- **`native-tls`**: TLS through the platform's native library.
- **`manager`**: `BrowserBuilder::managed` downloads, caches and spawns the
  driver for the browser in the capabilities. The browser itself must be
  installed.

## Limitations

WebDriver cannot report the status or headers of a navigation, nor send
custom request headers or bodies. Responses are always `200 OK` with an HTML
content type, and only `GET` requests are supported. Use `spire-reqwest` for
plain HTTP.

## Contributing

This crate follows the same contribution guidelines as the main spire project.
Please see [CONTRIBUTING.md](../../CONTRIBUTING.md) for details.

## License

This project is licensed under the MIT License - see the
[LICENSE.txt](../../LICENSE.txt) file for details.
