# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- N/A

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

## [0.1.1] - 2025-11-30

### Added

- **Error Handling Improvements**
  - Added `ErrorKind` enum for error categorization with variants: `Http`, `Dataset`, `Worker`, `Backend`, `Context`, `Io`, `Timeout`, `Other`
  - Restructured `Error` type with proper error source chaining using `#[source]` from thiserror
  - Added `BoxError` type alias for `Box<dyn std::error::Error + Send + Sync>`
  - New error constructors: `Error::new()`, `Error::with_source()`, `Error::from_boxed()`

- **Observability Features**
  - Added feature-gated tracing support via `trace` feature flag
  - Instrumented `Runner::run()`, `Runner::run_once()`, and `Runner::call_service()` with structured logging
  - Added trace instrumentation to `Runner::notify()`, `Runner::apply_defer()`, and `Runner::apply_abort()`
  - Tracing spans include request metadata (URI, method, depth), timing metrics, and request throughput

- **Graceful Shutdown**
  - Added `CancellationToken` support for graceful shutdown
  - New `Client::shutdown_token()` method to obtain shutdown token
  - Shutdown monitor task automatically stops request processing when token is cancelled
  - Tracing logs for shutdown events

- **Dataset Bulk Operations**
  - New `DatasetBulkExt` trait for efficient batch operations
  - Added `write_bulk()` for writing multiple items at once
  - Added `read_bulk()` for reading up to N items
  - Added `read_all()` for draining entire dataset
  - All bulk methods have default implementations for backwards compatibility
  - Blanket implementation for all `Dataset` types

- **Documentation**
  - Added `CONTRIBUTING.md` with development guidelines, coding standards, and contribution process
  - Added `CHANGELOG.md` for tracking project changes
  - Improved documentation in `extract/select.rs` with comprehensive descriptions for `AttrTag`, `AttrData`, `Select` trait, and `Elements<T>`
  - Enhanced documentation in `extract/client.rs` with module-level docs and examples for `Html` extractor
  - Enhanced documentation in `extract/driver.rs` with module-level docs and details for `View` extractor

- **Middleware Support**
  - Created `spire/middleware` module with comprehensive documentation
  - Explained tower integration and how to use middleware with routers

- **Backend Feature Flags**
  - Added `reqwest` feature flag for HTTP client backend support
  - Added `fantoccini` feature flag for WebDriver/browser automation backend support
  - Re-exported backend implementations as `reqwest_backend` and `fantoccini_backend`
  - Updated lib.rs with comprehensive feature flag documentation

- **GitHub Workflows**
  - Added `build.yml` workflow for multi-platform testing, formatting, clippy, docs, and code coverage
  - Added `release.yml` workflow for automated releases and crates.io publishing
  - Added `security.yml` workflow for security auditing, dependency review, Semgrep, and CodeQL analysis
  - Added `deny.toml` configuration for cargo-deny
  - Added `rustfmt.toml` configuration

### Changed

- **Version Update**: Changed version from `0.1.1-rc.1` to `0.1.1`
- **Feature Flag Refactoring**: Removed internal `client` and `driver` feature flags, replaced with direct `reqwest` and `fantoccini` backend features
- **Router Performance**: Implemented `Arc<TagRouter>` with copy-on-write pattern to reduce cloning overhead
- **Documentation Improvements**: Fixed TODO comments in routing documentation, replaced `# Errors` with `# Panics` where appropriate
- Improved `Data<T>` ergonomics by replacing `into_inner()` with `as_dataset()` for better API clarity
- Updated documentation across core modules with better examples and explanations
- Enhanced error messages throughout the codebase with more context
- Improved panic messages in `TagRouter::route()` with better context about route conflicts

### Fixed

- Fixed lifetime issues in extractors by adding `'static` bounds to generic parameters
- Fixed unused variable warnings in extractor implementations

### Dependencies

- Added `tokio` to `spire-core` dependencies for shutdown support
- Added `tokio-util` (v0.7) to workspace and `spire-core` for `CancellationToken`
- Added `spire-reqwest` and `spire-fantoccini` to workspace dependencies

## [0.1.1-rc.1] - Previous Release

### Added
- Initial release candidate with core functionality
- `Dataset` trait and `InMemDataset` implementation
- `Client` and `Runner` for request processing
- `Backend` and `Worker` traits
- Request/response context types
- Signal-based flow control
- Type-erased dataset utilities
- Reqwest and Fantoccini backend implementations

[Unreleased]: https://github.com/spire-rs/spire/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/spire-rs/spire/releases/tag/v0.1.1
[0.1.1-rc.1]: https://github.com/spire-rs/spire/releases/tag/v0.1.1-rc.1
