# spire-macros

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]

**Check out other `spire` projects [here](https://github.com/spire-rs).**

[action-badge]: https://img.shields.io/github/actions/workflow/status/spire-rs/spire/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/spire-rs/spire/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/spire-macros.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/spire-macros
[docs-badge]: https://img.shields.io/docsrs/spire-macros?logo=Docs.rs&style=flat-square
[docs-url]: http://docs.rs/spire-macros

Derive macros for Spire: HTML extraction, typed tags, revisit intervals and
router state.

## Overview

- **`Select`** turns CSS selectors on the fields of a struct into extraction
  from HTML.
- **`Tags`** makes an enum of unit variants the crawl's tags, checked by the
  compiler.
- **`Revisit`** gives each tag variant an interval to crawl its pages again
  after.
- **`FromRef`** lets a handler take only the parts of a router state it needs.

Their errors point at the code at fault, and malformed attributes are
rejected rather than ignored.

## Installation

This crate is not used directly. Enable the `macros` feature of `spire`:

```toml
[dependencies]
spire = { version = "0.11", features = ["macros"] }
```

The derives are then available next to the traits of the same names:
`spire::extract::html::Select`, `spire::crawler::Tags`,
`spire::crawler::Revisit` and `spire::extract::FromRef`.

## Usage

### `Select`

Derives `spire::extract::html::Select`, turning CSS selectors on the fields of
a struct into extraction from HTML:

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
- Every field needs `#[select("css")]`, `#[select("css", attr = "name")]`, or
  `#[select(attr = "name")]` for an attribute of the matched element itself.
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

### `Revisit`

Derives `spire::crawler::Revisit` for an enum of unit variants: a variant with
`#[revisit("1d")]` is crawled again that long after, one without is crawled
once. Enable it on a frontier with `.revisit::<Page>()`:

```rust,ignore
use spire::crawler::{Revisit, Tags};

#[derive(Debug, Clone, Copy, Tags, Revisit)]
enum Page {
    #[revisit("1h")]
    Listing,
    #[revisit("1d")]
    Product,
    // No attribute: crawled once.
    About,
}
```

- Intervals combine days (`d`), hours (`h`), minutes (`m`) and seconds (`s`),
  such as `"90s"`, `"30m"`, `"1h30m"` or `"1d"`.
- Intervals must be above zero, and are checked at compile time.

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
- Two fields of the same type conflict: `#[from_ref(skip)]` leaves all but one
  out.
- The struct must have named fields and no generic parameters.

## Learn more

- The book's [HTML](https://spire-rs.github.io/spire/html.html),
  [Routing and handlers](https://spire-rs.github.io/spire/routing.html),
  [Recrawls](https://spire-rs.github.io/spire/recrawls.html) and
  [State](https://spire-rs.github.io/spire/state.html) pages, and
  [Feature flags](https://spire-rs.github.io/spire/features.html#spire-facade)
- The [API documentation](https://docs.rs/spire-macros)

## Contributing

See [CONTRIBUTING.md](../../CONTRIBUTING.md) for development setup and
guidelines.

## License

MIT License, see [LICENSE](../../LICENSE).
