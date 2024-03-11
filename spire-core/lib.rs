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
/// [`Request`]: context::Request
#[derive(Debug)]
pub struct Error {
    // TODO: thiserror.
    inner: BoxError,
}

impl Error {
    pub fn new<T>(error: T) -> Self
    where
        T: Into<BoxError>,
    {
        let inner: BoxError = error.into();
        Self { inner }
    }

    pub fn into_inner(self) -> BoxError {
        self.inner
    }
}

impl<T> From<T> for Error
where
    T: Into<BoxError>,
{
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

/// Specialized [`Result`] type for [`Request`] processing.
///
/// [`Result`]: std::result::Result
/// [`Request`]: context::Request
pub type Result<T, E = Error> = std::result::Result<T, E>;
