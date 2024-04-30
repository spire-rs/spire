#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("./README.md")]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

#[doc(no_inline)]
pub use async_trait::async_trait;

pub use process::Client;

use crate::context::{IntoSignal, Signal, TagQuery};

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
/// [`Request`]: context::Request
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct Error {
    #[from]
    inner: BoxError,
    // fatal: bool,
}

impl Error {
    /// Creates a new [`Error`] from a boxable error.
    pub fn new(error: impl Into<BoxError>) -> Self {
        let inner: BoxError = error.into();
        Self { inner }
    }

    /// Returns inner error.
    #[inline]
    #[must_use]
    pub fn into_inner(self) -> BoxError {
        self.inner
    }
}

impl From<http::Error> for Error {
    fn from(error: http::Error) -> Self {
        todo!()
    }
}

// TODO: Use Error::signal.
impl IntoSignal for Error {
    fn into_signal(self) -> Signal {
        Signal::Fail(TagQuery::Owner, self.into_inner())
    }
}

/// Specialized [`Result`] type for [`Request`] processing.
///
/// [`Result`]: std::result::Result
/// [`Request`]: context::Request
pub type Result<T, E = Error> = std::result::Result<T, E>;
