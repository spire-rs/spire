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



## Usage

This crate is typically not used directly. Instead, use the main `spire` crate which re-exports these types:

```toml
[dependencies]
spire = "0.2.0"
```

If you're implementing custom backends or extending spire's functionality, you may need to depend on `spire-core` directly:

```toml
[dependencies]
spire-core = "0.2.0"
```

## Feature Flags

- **`tracing`** - Enables tracing support
- **`metric`** - Enables metrics collection

## Example

Implementing a custom backend:

```rust,ignore
use spire_core::backend::{Backend, Client};
use spire_core::context::{Request, Response};
use spire_core::Result;

#[derive(Clone)]
struct MyBackend;

#[async_trait::async_trait]
impl Backend for MyBackend {
    type Client = MyClient;
    
    async fn connect(&self) -> Result<Self::Client> {
        Ok(MyClient)
    }
}

struct MyClient;

#[async_trait::async_trait]
impl Client for MyClient {
    async fn resolve(self, req: Request) -> Result<Response> {
        // Custom request handling
        todo!()
    }
}
```

## License

This project is licensed under the MIT License - see the [LICENSE](../LICENSE) file for details.
