//! Sitemap extraction and parsed records.
//!
//! The [`Sitemap`] extractor holds records from the
//! [sitemapo](https://docs.rs/sitemapo) parser.
//!
//! A [`Record`] is an [`Entry`] for a page, with its image, video, news and
//! alternate-language extensions, or an [`IndexEntry`] for a sitemap an
//! index lists, which sitemapo names `Sitemap`.

mod parse;

pub use sitemapo::Record;
pub use sitemapo::record::{Sitemap as IndexEntry, *};

pub use self::parse::Sitemap;
