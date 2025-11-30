# spire-core

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]
[![Crate Coverage][coverage-badge]][coverage-url]

**Check out other `spire` projects [here](https://github.com/spire-rs).**

[action-badge]: https://img.shields.io/github/actions/workflow/status/spire-rs/spire/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/spire-rs/spire/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/spire-core.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/spire-core
[docs-badge]: https://img.shields.io/docsrs/spire-core?logo=Docs.rs&style=flat-square
[docs-url]: http://docs.rs/spire-core
[coverage-badge]: https://img.shields.io/codecov/c/github/spire-rs/spire?logo=codecov&logoColor=white&style=flat-square
[coverage-url]: https://app.codecov.io/gh/spire-rs/spire

Core types and traits for the spire web scraping framework.

## Overview

`spire-core` provides the foundational abstractions and types used throughout the spire ecosystem. This crate contains the core traits, error types, context structures, and dataset interfaces that power spire's flexible architecture.

## Key Components

### Backend Traits

- **`Backend`** - Trait for implementing custom backends (HTTP clients, browser automation, etc.)
- **`Client`** - Trait for executing requests and handling responses
- **`Worker`** - Trait for managing pooled resources

### Context Types

- **`Context`** - Request/response context that flows through the system
- **`Request`** - Represents a scraping request with URI and metadata
- **`Response`** - Backend-agnostic response wrapper
- **`Signal`** - Flow control signals (Continue, Defer, Abort)
- **`Tag`** - Type-safe routing tags

### Dataset Traits

- **`Dataset`** - Core trait for data storage and retrieval
- **`DatasetBulkExt`** - Extension trait for bulk operations
- **`InMemDataset`** - In-memory dataset implementations (queue, stack, set)
- **`Data`**, **`DataStream`**, **`DataSink`** - Typed dataset accessors

### Error Handling

- **`Error`** - Unified error type with source chaining
- **`ErrorKind`** - Error categorization (Http, Dataset, Worker, Backend, etc.)
- **`Result<T>`** - Type alias for `std::result::Result<T, Error>`

## Usage

This crate is typically not used directly. Instead, use the main `spire` crate which re-exports these types:

```toml
[dependencies]
spire = "0.1.1"
```

If you're implementing custom backends or extending spire's functionality, you may need to depend on `spire-core` directly:

```toml
[dependencies]
spire-core = "0.1.1"
```

## Feature Flags

- **`tracing`** - Enables tracing support
- **`trace`** - Enables detailed trace-level instrumentation
- **`metric`** - Enables metrics collection

## Example

Implementing a custom backend:

```rust
use spire_core::backend::{Backend, Client};
use spire_core::context::{Request, Response};
use spire_core::Result;

struct MyBackend;

impl Backend for MyBackend {
    type Client = MyClient;
    
    fn connect(&self) -> Result<Self::Client> {
        Ok(MyClient)
    }
}

struct MyClient;

#[async_trait::async_trait]
impl Client for MyClient {
    async fn send(&self, request: Request) -> Result<Response> {
        // Custom request handling
        todo!()
    }
}
```

## License

This project is licensed under the MIT License - see the [LICENSE](../LICENSE) file for details.
