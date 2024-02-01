#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("./README.md")]

pub mod collect;
pub mod crawler;
pub mod macros;

/// Unrecoverable failure during [`Spire`] execution.
///
/// This may be extended in the future so exhaustive matching is discouraged.
///
/// [`Spire`]: crate
#[derive(Debug)]
pub enum Error {}

/// A specialized [`Result`] type for [`Spire`] operations.
///
/// [`Result`]: std::result::Result
/// [`Spire`]: crate
pub type Result<T> = std::result::Result<T, Error>;
