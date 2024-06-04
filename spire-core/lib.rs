#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("./README.md")]
#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::convert::Infallible;

#[doc(no_inline)]
pub use async_trait::async_trait;

use crate::context::{IntoSignal, Signal, TagQuery};
pub use crate::process::Client;

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
#[must_use]
#[derive(Debug, thiserror::Error)]
#[error("{inner}")]
pub struct Error {
    inner: BoxError,
    fatal: Option<bool>,
    query: TagQuery,
}

impl Error {
    /// Creates a new [`Error`] from a boxable error.
    pub fn new(error: impl Into<BoxError>) -> Self {
        Self {
            inner: error.into(),
            fatal: None,
            query: TagQuery::Owner,
        }
    }

    /// Overrides the current [`TagQuery`].
    ///
    /// [`TagQuery::Owner`] by default.
    #[inline]
    pub fn with_query(mut self, query: impl Into<TagQuery>) -> Self {
        self.query = query.into();
        self
    }

    /// Marks the error as [`fatal`].
    #[inline]
    pub fn with_fatal(mut self, fatal: bool) -> Self {
        self.fatal = Some(fatal);
        self
    }

    /// Returns inner error.
    #[inline]
    #[must_use]
    pub fn into_inner(self) -> BoxError {
        self.inner
    }
}

impl From<BoxError> for Error {
    #[inline]
    fn from(value: BoxError) -> Self {
        Self::new(value)
    }
}

impl From<Infallible> for Error {
    #[inline]
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl From<http::Error> for Error {
    #[inline]
    fn from(error: http::Error) -> Self {
        todo!()
    }
}

impl IntoSignal for Error {
    fn into_signal(self) -> Signal {
        Signal::Fail(self.query, self.inner)
    }
}

/// Specialized [`Result`] type for the [`Request`] processing.
///
/// [`Result`]: std::result::Result
/// [`Request`]: context::Request
pub type Result<T, E = Error> = std::result::Result<T, E>;
