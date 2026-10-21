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

`spire-macros` provides derive macros that enable declarative HTML data
extraction in spire applications. This crate is automatically included when
using the `macros` feature flag in the main `spire` crate.

## Macros

### `Select`

Derives `spire::extract::Select`, turning CSS selectors on the fields of a
struct into extraction from HTML:

```rust,ignore
use spire::extract::{Elements, Select};

#[derive(Debug, Select)]
#[select("article.product")]
struct Product {
    #[select("h2")]
    name: String,
    #[select(".price")]
    price: Option<f64>,
    #[select("a.tag")]
    tags: Vec<String>,
    #[select("a.more", attr = "href")]
    link: String,
}

async fn handler(Elements(products): Elements<Product>) {
    for product in products {
        println!("{}: {:?}", product.name, product.price);
    }
}
```

- The container's `#[select("...")]` is optional: each match becomes one
  value. Without it, the whole document is one value.
- Every field needs `#[select("css")]`, `#[select("css", attr = "name")]`,
  or `#[select(attr = "name")]` for an attribute of the matched element
  itself.
- The field type decides how many matches it takes: `Option<T>` the first if
  any, `Vec<T>` all of them, any other `T` exactly the first. Values are the
  element's text, trimmed with whitespace collapsed, or the attribute, parsed
  with `FromStr`.

## Usage

This crate is typically not used directly. Instead, enable the `macros` feature
in the main `spire` crate:

```toml
[dependencies]
spire = { version = "0.2.0", features = ["macros"] }
```

The derive is then available as `spire::extract::Select`, next to the trait
of the same name.

## License

This project is licensed under the MIT License - see the
[LICENSE](../../LICENSE.txt) file for details.
