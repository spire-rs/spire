//! HTML: the [`Html`] extractor and its methods, extraction with [`Select`]
//! and [`Element`], a page's [`Meta`] and [`JsonLd`], its main content as
//! `Markdown` with the `markdown` feature, and the
//! [scraper](https://docs.rs/scraper) parser behind them.
//!
//! [`Html`] wraps a scraper document, to walk a page's tree directly when
//! its methods are not enough: [`scraper`] is re-exported here at the
//! version spire uses, e.g. for its `Selector` and `ElementRef`.

mod document;
mod links;
#[cfg(feature = "markdown")]
mod markdown;
mod meta;
pub(crate) mod select;
mod text;

pub use scraper;
#[cfg(feature = "macros")]
#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
pub use spire_macros::Select;

pub use self::document::Html;
pub use self::links::{LinkKind, Links, PageLink};
#[cfg(feature = "markdown")]
#[cfg_attr(docsrs, doc(cfg(feature = "markdown")))]
pub use self::markdown::{Markdown, MarkdownOptions};
pub use self::meta::{JsonLd, Meta};
pub use self::select::{Element, Select};
