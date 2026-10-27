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
[thirtyfour](https://github.com/stevepryde/thirtyfour) and WebDriver BiDi.

## Overview

`ThirtyfourBackend` keeps a pool of WebDriver sessions and hands one to every
request. Fetching a request navigates to it and returns the rendered page
source, so JavaScript-heavy pages scrape like any other. Through `Deref`, the
session also exposes the full `WebDriver` API for clicking, scrolling and
waiting on the page.

- **Real responses**: navigation goes through WebDriver BiDi, so responses
  carry the page's real status and headers, after redirects. Error statuses
  follow the same `StatusPolicy` as HTTP backends: `5xx`, `408` and `429`
  are retried, honouring `Retry-After`, and other `4xx` are rejected.
- **Any request**: requests are sent with their method, headers and body, so
  `POST` forms and custom headers work.
- **Isolation**: every request gets a tab in a fresh browser profile, so no
  cookies or storage carry over; `keep_session_state` shares one profile
  instead, e.g. for a logged-in crawl.
- **Identities**: with `Crawler::with_identities`, each identity gets a
  browser profile of its own, with its proxy, user agent and headers, and
  cookies kept between its requests; proxies asking for credentials get
  them.
- **Endpoints**: remote WebDriver servers (chromedriver, geckodriver, Selenium
  grids), rotated between in turn, or drivers downloaded and spawned
  automatically with the `manager` feature.
- **Errors**: WebDriver failures map to `spire_core::ErrorKind`s, so timeouts
  and navigation failures are retried while page bugs are not.

## Usage

Enable the `thirtyfour` feature of the main `spire` crate, or depend on this
crate directly to use the backend with `spire-core`:

```toml
[dependencies]
spire = { version = "0.7.0", features = ["thirtyfour"] }
```

```rust,no_run
use std::time::Duration;

use spire_thirtyfour::ThirtyfourBackend;

async fn example() -> spire_core::Result<()> {
    let backend = ThirtyfourBackend::builder()
        .remote("http://localhost:4444")
        .max_sessions(4)
        .page_load_timeout(Duration::from_secs(20))
        .build()?;

    // Crawl with `spire::Crawler::new(backend.clone(), router)`, then quit the
    // sessions before going on.
    backend.close().await;
    Ok(())
}
```

Sessions start headless Chrome by default. Pass your own capabilities for
another browser or for browser flags:

```rust
use spire_thirtyfour::ThirtyfourBackend;
use spire_thirtyfour::thirtyfour::DesiredCapabilities;

let builder = ThirtyfourBackend::builder()
    .remote("http://localhost:4444")
    .capabilities(DesiredCapabilities::firefox());
```

## Features

- **`rustls-tls`** (default): TLS to the WebDriver server through rustls.
- **`native-tls`**: TLS through the platform's native library.
- **`manager`**: `ThirtyfourBuilder::managed` downloads, caches and spawns the
  driver for the browser in the capabilities. The browser itself must be
  installed.

## Requirements

The WebDriver server must support WebDriver BiDi, as current chromedriver and
geckodriver do; starting a session on one without it fails. A response's body
is the rendered document serialized as HTML, so its content type is always
`text/html; charset=utf-8`, whatever the server sent.

The tests in `tests/bidi.rs` run against a real browser when `SPIRE_WEBDRIVER`
names a WebDriver server, e.g. `SPIRE_WEBDRIVER=http://127.0.0.1:4444`.

## Contributing

This crate follows the same contribution guidelines as the main spire project.
Please see [CONTRIBUTING.md](../../CONTRIBUTING.md) for details.

## License

This project is licensed under the MIT License; see the
[LICENSE](../../LICENSE) file for details.
