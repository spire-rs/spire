//! Error and result types.
//!
//! An [`Error`] carries a classified [`ErrorKind`], a message and an optional
//! source. The kind drives retry decisions: the crawler retries a failed
//! request when [`Error::is_retryable`] holds.

use std::convert::Infallible;
use std::fmt;
use std::time::Duration;

use ::http::{StatusCode, Uri};

/// A result whose error defaults to [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// A boxed, thread-safe source error.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Classification of a failure, so callers can branch without matching on
/// messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// The input was invalid: a malformed URL, request or configuration.
    Invalid,
    /// The transfer failed, or the remote answered with a status worth
    /// retrying, such as `429` or `503`.
    Http,
    /// The remote refused the request in a way retrying won't fix, such as
    /// `404` or `403`.
    Rejected,
    /// The operation timed out.
    Timeout,
    /// A backend failed, e.g. while connecting or driving a browser.
    Backend,
    /// A worker or handler failed.
    Worker,
    /// Reading from or writing to a dataset failed.
    Dataset,
    /// Reading from or writing to the frontier failed.
    Frontier,
    /// The operation was cancelled.
    Cancelled,
}

impl ErrorKind {
    /// Returns the kind as a lowercase string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Invalid => "invalid",
            Self::Http => "http",
            Self::Rejected => "rejected",
            Self::Timeout => "timeout",
            Self::Backend => "backend",
            Self::Worker => "worker",
            Self::Dataset => "dataset",
            Self::Frontier => "frontier",
            Self::Cancelled => "cancelled",
        }
    }

    /// Whether an operation failing with this kind is worth retrying. Only
    /// transient failures are retryable.
    #[must_use]
    pub const fn is_retryable(self) -> bool {
        matches!(self, Self::Http | Self::Timeout | Self::Backend)
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An error carrying a classified [`ErrorKind`], a message, and an optional
/// source.
#[derive(Debug, thiserror::Error)]
pub struct Error {
    kind: ErrorKind,
    message: String,
    status: Option<StatusCode>,
    retry_after: Option<Duration>,
    uri: Option<Box<Uri>>,
    #[source]
    source: Option<BoxError>,
}

impl Error {
    /// Creates an error of the given [`ErrorKind`] with an empty message.
    #[must_use]
    pub fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            message: String::new(),
            status: None,
            retry_after: None,
            uri: None,
            source: None,
        }
    }

    /// Wraps an arbitrary error under the given [`ErrorKind`].
    ///
    /// If the error already is an [`Error`], it is returned unchanged and
    /// keeps its own kind.
    pub fn from_boxed(kind: ErrorKind, error: impl Into<BoxError>) -> Self {
        match error.into().downcast::<Self>() {
            Ok(error) => *error,
            Err(error) => Self::new(kind).with_source(error),
        }
    }

    /// Creates an [`Invalid`](ErrorKind::Invalid) error with a message.
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Invalid).with_message(message)
    }

    /// Creates a [`Backend`](ErrorKind::Backend) error with a message.
    pub fn backend(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Backend).with_message(message)
    }

    /// Creates a [`Worker`](ErrorKind::Worker) error with a message.
    pub fn worker(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Worker).with_message(message)
    }

    /// Replaces the message.
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// Attaches a source error.
    #[must_use]
    pub fn with_source(mut self, source: impl Into<BoxError>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Attaches the response status that caused the error.
    #[must_use]
    pub const fn with_status(mut self, status: StatusCode) -> Self {
        self.status = Some(status);
        self
    }

    /// Attaches the URI of the request that failed.
    #[must_use]
    pub fn with_uri(mut self, uri: Uri) -> Self {
        self.uri = Some(Box::new(uri));
        self
    }

    /// Asks for the failed request's tag to be paused for `delay` before it
    /// is retried, e.g. from a `Retry-After` header.
    #[must_use]
    pub const fn with_retry_after(mut self, delay: Duration) -> Self {
        self.retry_after = Some(delay);
        self
    }

    /// Returns the [`ErrorKind`].
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns the message, which may be empty.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the response status that caused the error, if any.
    #[must_use]
    pub const fn status(&self) -> Option<StatusCode> {
        self.status
    }

    /// Returns the URI of the request that failed, if known.
    #[must_use]
    pub fn uri(&self) -> Option<&Uri> {
        self.uri.as_deref()
    }

    /// Returns how long to wait before retrying, if the remote said so.
    #[must_use]
    pub const fn retry_after(&self) -> Option<Duration> {
        self.retry_after
    }

    /// Whether the failed operation is worth retrying.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        self.kind.is_retryable()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.message.is_empty(), &self.source) {
            (false, _) => f.write_str(&self.message),
            (true, Some(source)) => write!(f, "{}: {source}", self.kind),
            (true, None) => write!(f, "{} error", self.kind),
        }
    }
}

impl From<Infallible> for Error {
    fn from(never: Infallible) -> Self {
        match never {}
    }
}

impl From<::http::Error> for Error {
    fn from(error: ::http::Error) -> Self {
        Self::invalid("invalid request").with_source(error)
    }
}

impl From<::http::uri::InvalidUri> for Error {
    fn from(error: ::http::uri::InvalidUri) -> Self {
        Self::invalid("invalid uri").with_source(error)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::*;

    #[test]
    fn display_prefers_message() {
        let error = Error::backend("pool exhausted");
        assert_eq!(error.to_string(), "pool exhausted");
        assert_eq!(Error::new(ErrorKind::Timeout).to_string(), "timeout error");
    }

    #[test]
    fn from_boxed_keeps_existing_kind() {
        let inner = Error::new(ErrorKind::Timeout);
        let error = Error::from_boxed(ErrorKind::Backend, inner);
        assert_eq!(error.kind(), ErrorKind::Timeout);

        let error = Error::from_boxed(ErrorKind::Backend, std::io::Error::other("io"));
        assert_eq!(error.kind(), ErrorKind::Backend);
        assert!(error.source().is_some());
    }

    #[test]
    fn retryable_kinds() {
        assert!(Error::new(ErrorKind::Http).is_retryable());
        assert!(!Error::new(ErrorKind::Rejected).is_retryable());
        assert!(!Error::invalid("bad").is_retryable());
    }
}
