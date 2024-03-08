#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("./README.md")]

pub use process::Daemon;

pub mod backend;
pub mod context;
pub mod dataset;
mod process;

/// Type alias for a type-erased [`Error`] type.
///
/// [`Error`]: std::error::Error
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Unrecoverable failure during [`Request`] processing.
///
/// This may be extended in the future so exhaustive matching is discouraged.
///
/// [`Request`]: context::Request
pub enum Error {
    Backend(BoxError),
    Dataset(BoxError),
}

/// A specialized [`Result`] type for [`Request`] processing.
///
/// [`Result`]: std::result::Result
/// [`Request`]: context::Request
pub type Result<T> = std::result::Result<T, Error>;
