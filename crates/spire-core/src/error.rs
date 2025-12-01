use std::convert::Infallible;
use std::time::Duration;
use std::{fmt, io};

use crate::context::{FlowControl, IntoFlowControl, TagQuery};

/// Type alias for a type-erased [`Error`] type.
///
/// [`Error`]: std::error::Error
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Error kind representing the category of error that occurred.
///
/// This enum categorizes errors by their source and nature, making it easier
/// to handle different error scenarios programmatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// HTTP-related errors (invalid requests, connection failures, etc.)
    Http,

    /// Dataset operation errors (read/write failures, serialization errors)
    Dataset,

    /// Worker processing errors (business logic failures)
    Worker,

    /// Backend initialization or client creation errors
    Backend,

    /// Context or request processing errors
    Context,

    /// I/O errors (file system, network)
    Io,

    /// Timeout errors
    Timeout,

    /// Other unclassified errors
    Other,
}

impl ErrorKind {
    /// Returns a string representation of the error kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Dataset => "dataset",
            Self::Worker => "worker",
            Self::Backend => "backend",
            Self::Context => "context",
            Self::Io => "io",
            Self::Timeout => "timeout",
            Self::Other => "other",
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Unrecoverable failure during [`Request`] processing.
///
/// `Error` provides structured error information including:
/// - Error kind for categorization
/// - Optional source error for error chains
/// - Optional tag query for selective task termination
///
/// # Examples
///
/// ## Creating Errors
///
/// ```no_run
/// use spire_core::{Error, ErrorKind};
///
/// // From a message
/// let err = Error::new(ErrorKind::Worker, "failed to parse response");
///
/// // With a source error
/// let err = Error::with_source(
///     ErrorKind::Http,
///     "request failed",
///     Box::new(std::io::Error::from(std::io::ErrorKind::ConnectionReset))
/// );
/// ```
///
/// ## Error Handling
///
/// ```no_run
/// use spire_core::{Error, ErrorKind};
///
/// # let result: Result<(), Error> = Err(Error::new(ErrorKind::Timeout, "timeout"));
/// match result {
///     Err(e) if e.kind() == ErrorKind::Timeout => {
///         // Retry on timeout
///     }
///     Err(e) => {
///         // Log error with source chain
///         eprintln!("Error: {}", e);
///         if let Some(source) = e.source() {
///             eprintln!("Caused by: {}", source);
///         }
///     }
///     Ok(v) => { /* ... */ }
/// }
/// ```
///
/// [`Request`]: crate::context::Request
#[must_use]
#[derive(thiserror::Error)]
pub struct Error {
    kind: ErrorKind,
    message: String,
    #[source]
    source: Option<BoxError>,
    query: Option<TagQuery>,
}

impl Error {
    /// Creates a new [`Error`] with the given kind and message.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use spire_core::{Error, ErrorKind};
    ///
    /// let err = Error::new(ErrorKind::Dataset, "failed to write data");
    /// ```
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
            query: None,
        }
    }

    /// Creates a new [`Error`] with the given kind, message, and source error.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use spire_core::{Error, ErrorKind};
    ///
    /// let io_err = std::io::Error::from(std::io::ErrorKind::NotFound);
    /// let err = Error::with_source(
    ///     ErrorKind::Dataset,
    ///     "failed to read file",
    ///     Box::new(io_err)
    /// );
    /// ```
    pub fn with_source(kind: ErrorKind, message: impl Into<String>, source: BoxError) -> Self {
        Self {
            kind,
            message: message.into(),
            source: Some(source),
            query: None,
        }
    }

    /// Creates a new [`Error`] from a boxable error with automatic kind detection.
    ///
    /// Attempts to determine the error kind based on the error type.
    pub fn from_boxed(error: impl Into<BoxError>) -> Self {
        let boxed = error.into();
        let message = boxed.to_string();

        // Try to infer kind from error type name
        let kind = if message.contains("http") || message.contains("request") {
            ErrorKind::Http
        } else if message.contains("io") || message.contains("file") {
            ErrorKind::Io
        } else if message.contains("timeout") {
            ErrorKind::Timeout
        } else {
            ErrorKind::Other
        };

        Self {
            kind,
            message,
            source: Some(boxed),
            query: None,
        }
    }

    /// Returns the error kind.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use spire_core::{Error, ErrorKind};
    ///
    /// let err = Error::new(ErrorKind::Http, "request failed");
    /// assert_eq!(err.kind(), ErrorKind::Http);
    /// ```
    #[inline]
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns the error message.
    #[inline]
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Overrides the current [`TagQuery`].
    ///
    /// Terminates all collector tasks with matching [`Tag`]s.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use spire_core::{Error, ErrorKind};
    /// use spire_core::context::TagQuery;
    ///
    /// let err = Error::new(ErrorKind::Worker, "failed")
    ///     .with_query(TagQuery::Owner);
    /// ```
    ///
    /// [`Tag`]: crate::context::Tag
    #[inline]
    pub fn with_query(mut self, query: TagQuery) -> Self {
        self.query = Some(query);
        self
    }

    /// Returns the tag query if set.
    #[inline]
    #[must_use]
    pub const fn query(&self) -> Option<&TagQuery> {
        self.query.as_ref()
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("Error");
        debug
            .field("kind", &self.kind)
            .field("message", &self.message);

        if let Some(ref source) = self.source {
            debug.field("source", source);
        }

        if let Some(ref query) = self.query {
            debug.field("query", query);
        }

        debug.finish()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.kind, self.message)
    }
}

impl From<BoxError> for Error {
    #[inline]
    fn from(value: BoxError) -> Self {
        Self::from_boxed(value)
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
        Self::with_source(ErrorKind::Http, "HTTP error", Box::new(error))
    }
}

impl From<io::Error> for Error {
    #[inline]
    fn from(error: io::Error) -> Self {
        Self::with_source(ErrorKind::Io, "I/O error", Box::new(error))
    }
}

impl IntoFlowControl for Error {
    fn into_flow_control(self) -> FlowControl {
        match self.query {
            Some(query) => {
                let message = if let Some(source) = self.source {
                    source
                } else {
                    // Convert string message to io::Error which implements std::error::Error
                    Box::new(io::Error::other(self.message)) as BoxError
                };
                FlowControl::Fail(query, message)
            }
            None => FlowControl::Hold(TagQuery::Owner, Duration::default()),
        }
    }
}
