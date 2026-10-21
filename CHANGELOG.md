# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `View` extractor now wraps the browser `WebDriver` (`spire::extract::driver::View`).

### Changed

- **Breaking:** TLS feature `rustls-tls` renamed to `rustls` on `spire`, `spire-reqwest` and
  `spire-thirtyfour`, matching upstream `reqwest` 0.13 and `thirtyfour` 0.38.
- Updated `reqwest` to 0.13, `thirtyfour` to 0.38, `deadpool` to 0.13 and `syn` to 3.0.
- MSRV raised to 1.95.

### Deprecated

- N/A

### Removed

- N/A

### Fixed

- `spire-core` declared no `tokio` features and failed to build outside the workspace.
- `spire-reqwest` dropped request and response bodies; both are now streamed through.

### Security

- N/A

## [0.2.0] - 2025-12-01

### Added

- Initial release of the Spire web scraping framework
- Flexible architecture built on tokio and tower
- Multiple backend support (reqwest for HTTP, thirtyfour for browser automation)
- Type-safe routing with tag-based request handling
- Ergonomic extractors for data extraction from requests
- Built-in middleware support via tower integration
- Graceful shutdown with cancellation token support
- Dataset operations for managing scraped data
- Comprehensive error handling with `ErrorKind` enum
- Optional tracing support for observability
- Feature flags for modular functionality
- Comprehensive documentation and examples

### Changed

- N/A

### Deprecated

- N/A

### Removed

- N/A

### Fixed

- N/A

### Security

- N/A

## [0.1.0] - 2025-12-01

### Added

- Initial placeholder release for name reservation

[Unreleased]: https://github.com/spire-rs/spire/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/spire-rs/spire/releases/tag/v0.2.0
[0.1.0]: https://github.com/spire-rs/spire/releases/tag/v0.1.0
