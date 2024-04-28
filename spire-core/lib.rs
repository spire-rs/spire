#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("./README.md")]

pub use daemon::Daemon;

pub mod backend;
pub mod context;
mod daemon;
pub mod dataset;

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
}

impl Error {
    /// Creates a new [`Error`] from a boxable error.
    pub fn new(error: impl Into<BoxError>) -> Self {
        let inner: BoxError = error.into();
        Self { inner }
    }

    /// Returns inner error.
    pub fn into_inner(self) -> BoxError {
        self.inner
    }
}

impl From<http::Error> for Error {
    fn from(error: http::Error) -> Self {
        todo!()
    }
}

/// Specialized [`Result`] type for [`Request`] processing.
///
/// [`Result`]: std::result::Result
/// [`Request`]: context::Request
pub type Result<T, E = Error> = std::result::Result<T, E>;
