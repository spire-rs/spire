//! HTML extraction, typed selection and page metadata.
//!
//! Extract documents with [`Html`], typed data with [`Select`] and
//! [`Element`], and metadata with [`Meta`] and [`JsonLd`]. The `markdown`
//! feature provides `Markdown` for a page's main content. Parsing uses
//! [scraper](https://docs.rs/scraper).
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
mod visibility;

pub use scraper;
#[cfg(feature = "macros")]
#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
pub use spire_macros::Select;

pub use self::document::Html;
pub use self::links::{LinkKind, Links, PageLink, Visibility};
#[cfg(feature = "markdown")]
#[cfg_attr(docsrs, doc(cfg(feature = "markdown")))]
pub use self::markdown::{Markdown, MarkdownOptions};
pub use self::meta::{JsonLd, Meta};
pub use self::select::{Element, Select};
