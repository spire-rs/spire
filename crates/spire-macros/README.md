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

Derives `spire::extract::html::Select`, turning CSS selectors on the fields of a
struct into extraction from HTML:

```rust,ignore
use spire::extract::html::{Element, Select};

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

async fn handler(Element(products): Element<Vec<Product>>) {
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

### `Tags`

Derives `spire::crawler::Tags` for an enum of unit variants, so tags are
checked by the compiler:

```rust,ignore
use spire::Router;
use spire::crawler::Tags;

#[derive(Debug, Clone, Copy, Tags)]
enum Page {
    List,
    ItemReviews,
    #[tag(rename = "item")]
    Detail,
}

let router = Router::new()
    .route(Page::List, list)
    .route(Page::ItemReviews, reviews)
    .route(Page::Detail, detail)
    .require::<Page>();
```

- Variants are named in `snake_case` (`ItemReviews` is `item_reviews`);
  `#[tag(rename_all = "...")]` on the enum picks another case, with serde's
  names (`kebab-case`, `camelCase`, ...), and `#[tag(rename = "...")]` on a
  variant names it outright.
- Persistent frontiers store tags as strings, so rename a variant with
  `#[tag(rename = "old_name")]` to keep its stored seeds routable.
- The enum is also an extractor: a handler serving several variants takes
  `page: Page` (or `Option<Page>`) as an argument.
- The enum must be `Copy` and have only unit variants with distinct names.

### `FromRef`

Derives `spire::extract::FromRef` for the type of every field of a router
state, so a handler takes only the parts it needs with `State`:

```rust,ignore
use spire::extract::{FromRef, State};

#[derive(Clone, FromRef)]
struct AppState {
    db: Db,
    #[from_ref(skip)]
    api_key: String,
}

async fn page(State(db): State<Db>) {}
```

- Each field is cloned, so keep large or shared parts behind an `Arc`.
- Two fields of the same type conflict: `#[from_ref(skip)]` leaves all but
  one out.
- The struct must have named fields and no generic parameters.

## Feature flags

This crate has no feature flags. Enable `macros` on `spire` to use its
`Select`, `FromRef`, `Tags` and `Revisit` derives through the public API.

## Usage

This crate is typically not used directly. Instead, enable the `macros` feature
in the main `spire` crate:

```toml
[dependencies]
spire = { version = "0.6.0", features = ["macros"] }
```

The derives are then available as `spire::extract::html::Select` and
`spire::crawler::Tags`, next to the traits of the same names.

## License

This project is licensed under the MIT License; see the
[LICENSE](../../LICENSE) file for details.
