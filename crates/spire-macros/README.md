# spire-macros

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]
[![Crate Coverage][coverage-badge]][coverage-url]

**Check out other `spire` projects [here](https://github.com/spire-rs).**

[action-badge]: https://img.shields.io/github/actions/workflow/status/spire-rs/spire/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/spire-rs/spire/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/spire-macros.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/spire-macros
[docs-badge]: https://img.shields.io/docsrs/spire-macros?logo=Docs.rs&style=flat-square
[docs-url]: http://docs.rs/spire-macros
[coverage-badge]: https://img.shields.io/codecov/c/github/spire-rs/spire?logo=codecov&logoColor=white&style=flat-square
[coverage-url]: https://app.codecov.io/gh/spire-rs/spire

Procedural macros for the spire web scraping framework.

## Overview

`spire-macros` provides derive macros that enable declarative HTML data extraction in spire applications. This crate is automatically included when using the `macros` feature flag in the main `spire` crate.

## Macros

### `Select` Derive Macro

The `Select` derive macro generates implementations of the `Select` trait, allowing you to declaratively extract structured data from HTML:

```rust,ignore
use spire::extract::Select;

#[derive(Debug, Select)]
struct Product {
    #[select(css = "h1.title")]
    name: String,
    
    #[select(css = ".price", attr = "data-price")]
    price: f64,
    
    #[select(css = ".description")]
    description: String,
}
```

This enables type-safe extraction in your handlers:

```rust,ignore
use spire::extract::Elements;

async fn handler(Elements(products): Elements<Vec<Product>>) {
    for product in products {
        println!("{}: ${}", product.name, product.price);
    }
}
```

## Usage

This crate is typically not used directly. Instead, enable the `macros` feature in the main `spire` crate:

```toml
[dependencies]
spire = { version = "0.2.0", features = ["macros"] }
```

The macros will then be available through the `spire` prelude:

```rust,ignore
use spire::prelude::*;
use spire::extract::Select;

#[derive(Select)]
struct MyData {
    // ...
}
```

## License

This project is licensed under the MIT License - see the [LICENSE](../LICENSE) file for details.
