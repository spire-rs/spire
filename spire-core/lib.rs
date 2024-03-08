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
