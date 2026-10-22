//! HTML: the [`Html`] extractor and its methods, extraction with [`Select`]
//! and [`Element`], a page's [`Meta`] and [`JsonLd`], and the
//! [scraper](https://docs.rs/scraper) parser behind them.
//!
//! [`Html`] wraps a scraper [`Document`], to walk a page's tree directly
//! when its methods are not enough; the rest of scraper's API, such as
//! [`ElementRef`], [`Selector`] and [`Node`], is re-exported here at the
//! version spire uses. scraper's own `Element`, an element's raw data, is
//! at [`node::Element`].

mod document;
mod meta;
pub(crate) mod select;
mod text;

pub use scraper::{Html as Document, *};
#[cfg(feature = "macros")]
#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
pub use spire_macros::Select;

pub use self::document::Html;
pub use self::meta::{JsonLd, Meta};
pub use self::select::{Element, Select};
