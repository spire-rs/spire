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

Browser automation backend for Spire.

This crate provides [`BrowserBackend`], a browser automation backend
implementation that integrates with the Spire web scraping framework using the
[thirtyfour](https://github.com/stevepryde/thirtyfour) WebDriver library.

## Overview

`spire-thirtyfour` provides a browser automation backend implementation for
Spire built on top of the [thirtyfour](https://github.com/stevepryde/thirtyfour)
WebDriver library.

The backend implements the `Backend` trait from `spire-core` and creates
`BrowserConnection` instances that implement the `Client` trait for performing
browser-based operations.

## Features

- **Built on thirtyfour**: Leverages the powerful thirtyfour WebDriver library
- **Spire Integration**: Seamlessly integrates with the Spire web scraping
  framework
- **Browser Automation**: Control real browsers for JavaScript-heavy websites
  and SPAs
- **Async/Await**: Fully async implementation for concurrent browser operations
- **TLS Support**: Choose between rustls (default) and native TLS
  implementations

## Usage

This crate is typically not used directly. Instead, enable the `thirtyfour`
feature in the main `spire` crate and refer to the
[spire documentation](https://docs.rs/spire) for usage examples and guides.

```toml
[dependencies]
spire = { version = "0.2.0", features = ["thirtyfour"] }
```

### TLS Configuration

By default, `spire-thirtyfour` uses rustls for TLS connections. You can choose
between different TLS implementations:

```toml
# Default: uses rustls-tls
[dependencies]
spire-thirtyfour = "0.2.0"

# Explicitly use rustls
[dependencies]
spire-thirtyfour = { version = "0.2.0", features = ["rustls-tls"], default-features = false }

# Use native TLS (system TLS library)
[dependencies]
spire-thirtyfour = { version = "0.2.0", features = ["native-tls"], default-features = false }
```

For advanced usage and custom configurations, see the
[API documentation](https://docs.rs/spire-thirtyfour).

## Prerequisites

Browser automation requires a WebDriver server. For Chrome:

```bash
# ChromeDriver is automatically managed by thirtyfour
# Ensure Chrome browser is installed and accessible
```

For other browsers, see the
[thirtyfour documentation](https://docs.rs/thirtyfour/) for setup instructions.

## Error Handling

All thirtyfour WebDriver errors are automatically converted to
`spire_core::Error` types. This includes browser connection errors, element
interaction failures, navigation timeouts, and JavaScript execution errors.

## Performance Considerations

- Browser instances are resource-intensive compared to HTTP clients.
- Connection pooling helps manage browser lifecycle and memory usage.
- Consider configuring appropriate pool sizes based on available system
  resources.
- Browser startup time can be significant; pool warm-up strategies may improve
  performance.
- JavaScript execution and page rendering add latency compared to HTTP-only
  scraping.
- Consider headless mode for better performance when visual rendering is not
  required.

## Contributing

This crate follows the same contribution guidelines as the main spire project.
Please see [CONTRIBUTING.md](../../CONTRIBUTING.md) for details.

## License

This project is licensed under the MIT License - see the
[LICENSE.txt](../../LICENSE.txt) file for details.
