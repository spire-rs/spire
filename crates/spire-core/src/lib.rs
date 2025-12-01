#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod backend;
pub mod context;
pub mod dataset;
mod error;
mod process;

#[doc(hidden)]
pub mod prelude;

#[doc(no_inline)]
pub use async_trait::async_trait;

pub use crate::error::{BoxError, Error, ErrorKind};
pub use crate::process::Client;

/// Specialized [`Result`] type for the [`Request`] processing.
///
/// [`Result`]: std::result::Result
/// [`Request`]: context::Request
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Tracing target for runner-related events.
#[cfg(feature = "tracing")]
pub(crate) const TRACING_TARGET_RUNNER: &str = "spire_core::runner";

/// Tracing target for backend-related events.
#[cfg(feature = "tracing")]
pub(crate) const TRACING_TARGET_BACKEND: &str = "spire_core::backend";
