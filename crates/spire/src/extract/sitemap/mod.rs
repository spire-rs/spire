//! The records a [`Sitemap`](crate::extract::Sitemap) holds, from the
//! [sitemapo](https://docs.rs/sitemapo) parser: [`Entry`] for a page, and
//! [`Sitemap`] for a sitemap an index lists, with their image, video, news
//! and alternate-language extensions.

pub(super) mod parse;

pub use sitemapo::Record;
pub use sitemapo::record::*;
