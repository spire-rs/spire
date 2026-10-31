#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

#[cfg(feature = "sqlite")]
mod error;

/// SQLite frontiers, datasets, seen sets and HTTP validators.
#[cfg(feature = "sqlite")]
#[cfg_attr(docsrs, doc(cfg(feature = "sqlite")))]
pub mod sqlite;

#[doc(no_inline)]
pub use sqlx;
