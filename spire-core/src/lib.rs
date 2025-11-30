#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]
// #![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::convert::Infallible;
use std::time::Duration;

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
    query: Option<TagQuery>,
}

impl Error {
    /// Creates a new [`Error`] from a boxable error.
    pub fn new(error: impl Into<BoxError>) -> Self {
        Self {
            inner: error.into(),
            query: None,
        }
    }

    /// Overrides the current [`TagQuery`].
    ///
    /// Terminates all collector tasks with matching [`Tag`]s.
    ///
    /// [`Tag`]: crate::context::Tag
    #[inline]
    pub fn with_query(mut self, query: TagQuery) -> Self {
        self.query = Some(query);
        self
    }

    /// Returns the inner error.
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
        Self::from(BoxError::from(error))
    }
}

impl IntoSignal for Error {
    fn into_signal(self) -> Signal {
        match self.query {
            Some(query) => Signal::Fail(query, self.inner),
            None => Signal::Hold(TagQuery::Owner, Duration::default()),
        }
    }
}

/// Specialized [`Result`] type for the [`Request`] processing.
///
/// [`Result`]: std::result::Result
/// [`Request`]: context::Request
pub type Result<T, E = Error> = std::result::Result<T, E>;

// TODO: better oneshot request client
