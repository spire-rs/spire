//! Enhanced error handling for browser automation operations.

use spire_core::{Error, ErrorKind};
use thirtyfour::error::WebDriverError;
use thiserror::Error;

/// Specific error types for browser operations.
#[derive(Debug, Error)]
pub enum BrowserError {
    /// Failed to connect to WebDriver server
    #[error("Failed to connect to WebDriver server at '{url}': {source}")]
    ConnectionFailed {
        /// WebDriver server URL that failed
        url: String,
        /// Underlying connection error
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// WebDriver server returned an error
    #[error("WebDriver error{}: {error}", context.as_ref().map(|c| format!(" during {}", c)).unwrap_or_default())]
    WebDriverError {
        /// The underlying WebDriver error
        #[source]
        error: Box<WebDriverError>,
        /// Additional context about the operation that failed
        context: Option<String>,
    },

    /// Browser session is invalid or expired
    #[error("Invalid browser session{}: {reason}", session_id.as_ref().map(|id| format!(" '{}'", id)).unwrap_or_default())]
    InvalidSession {
        /// Session ID if available
        session_id: Option<String>,
        /// Reason for invalidity
        reason: String,
    },

    /// Timeout occurred during operation
    #[error("Operation '{operation}' timed out after {duration_secs} seconds")]
    Timeout {
        /// Operation that timed out
        operation: String,
        /// Duration of the timeout
        duration_secs: u64,
    },

    /// Browser pool is exhausted or unavailable
    #[error("Browser pool exhausted: {active_connections}/{pool_size} connections active")]
    PoolExhausted {
        /// Number of browsers in the pool
        pool_size: usize,
        /// Number of active connections
        active_connections: usize,
    },

    /// Configuration error
    #[error("Configuration error{}: {message}", field.as_ref().map(|f| format!(" in '{}'", f)).unwrap_or_default())]
    Configuration {
        /// Description of the configuration issue
        message: String,
        /// Field name if applicable
        field: Option<String>,
    },

    /// Browser process management error
    #[error("Process '{operation}' failed{}: {message}", exit_code.map(|c| format!(" with exit code {}", c)).unwrap_or_default())]
    ProcessError {
        /// Process operation that failed
        operation: String,
        /// Exit code if available
        exit_code: Option<i32>,
        /// Process output/error message
        message: String,
    },

    /// Element interaction error
    #[error("Element operation '{operation}' failed{}: {reason}", selector.as_ref().map(|s| format!(" on selector '{}'", s)).unwrap_or_default())]
    ElementError {
        /// Type of element operation
        operation: String,
        /// Element selector if available
        selector: Option<String>,
        /// Underlying error
        reason: String,
    },

    /// Navigation error
    #[error("Navigation to '{url}' failed: {error_type}{}", details.as_ref().map(|d| format!(" ({})", d)).unwrap_or_default())]
    NavigationError {
        /// URL being navigated to
        url: String,
        /// Type of navigation error
        error_type: NavigationErrorType,
        /// Additional details
        details: Option<String>,
    },

    /// JavaScript execution error
    #[error("Script execution failed: {message}\nScript: {script}")]
    ScriptError {
        /// Script that failed
        script: String,
        /// Error message from browser
        message: String,
    },

    /// Resource error (file not found, download failed, etc.)
    #[error("Resource error for {resource_type} '{identifier}': {reason}")]
    ResourceError {
        /// Resource type
        resource_type: String,
        /// Resource identifier (URL, path, etc.)
        identifier: String,
        /// Error description
        reason: String,
    },

    /// Capability error
    #[error("Capability '{capability}' error with value '{value}': {reason}")]
    CapabilityError {
        /// Capability name
        capability: String,
        /// Value that was problematic
        value: String,
        /// Error reason
        reason: String,
    },

    /// Generic browser operation error
    #[error("Operation '{operation}' failed: {message}")]
    OperationFailed {
        /// Operation name
        operation: String,
        /// Error message
        message: String,
    },
}

/// Types of navigation errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NavigationErrorType {
    /// URL is malformed or invalid
    #[error("Invalid URL")]
    InvalidUrl,
    /// Network error (DNS, connection refused, etc.)
    #[error("Network error")]
    NetworkError,
    /// HTTP error (404, 500, etc.)
    #[error("HTTP error {0}")]
    HttpError(u16),
    /// SSL/TLS certificate error
    #[error("Certificate error")]
    CertificateError,
    /// Timeout during navigation
    #[error("Navigation timeout")]
    Timeout,
    /// Navigation was blocked
    #[error("Navigation blocked")]
    Blocked,
    /// Unknown navigation failure
    #[error("Unknown navigation error")]
    Unknown,
}

impl BrowserError {
    /// Creates a connection failed error.
    pub fn connection_failed(
        url: impl Into<String>,
        source: impl Into<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        Self::ConnectionFailed {
            url: url.into(),
            source: source.into(),
        }
    }

    /// Creates a WebDriver error with optional context.
    pub fn webdriver(error: WebDriverError, context: Option<String>) -> Self {
        Self::WebDriverError {
            error: Box::new(error),
            context,
        }
    }

    /// Creates an invalid session error.
    pub fn invalid_session(session_id: Option<String>, reason: impl Into<String>) -> Self {
        Self::InvalidSession {
            session_id,
            reason: reason.into(),
        }
    }

    /// Creates a new timeout error.
    #[must_use]
    pub fn timeout(operation: impl Into<String>, duration_secs: u64) -> Self {
        Self::Timeout {
            operation: operation.into(),
            duration_secs,
        }
    }

    /// Creates a pool exhausted error.
    pub fn pool_exhausted(pool_size: usize, active_connections: usize) -> Self {
        Self::PoolExhausted {
            pool_size,
            active_connections,
        }
    }

    /// Creates a configuration error.
    pub fn configuration(message: impl Into<String>, field: Option<String>) -> Self {
        Self::Configuration {
            message: message.into(),
            field,
        }
    }

    /// Creates a process error.
    pub fn process_error(
        operation: impl Into<String>,
        exit_code: Option<i32>,
        message: impl Into<String>,
    ) -> Self {
        Self::ProcessError {
            operation: operation.into(),
            exit_code,
            message: message.into(),
        }
    }

    /// Creates an element error.
    pub fn element_error(
        operation: impl Into<String>,
        selector: Option<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::ElementError {
            operation: operation.into(),
            selector,
            reason: reason.into(),
        }
    }

    /// Creates a navigation error.
    pub fn navigation_error(
        url: impl Into<String>,
        error_type: NavigationErrorType,
        details: Option<String>,
    ) -> Self {
        Self::NavigationError {
            url: url.into(),
            error_type,
            details,
        }
    }

    /// Creates a script error.
    #[must_use]
    pub fn javascript_error(script: impl Into<String>, error: impl Into<String>) -> Self {
        Self::ScriptError {
            script: script.into(),
            message: error.into(),
        }
    }

    /// Creates a resource error.
    #[must_use]
    pub fn resource_error(
        resource_type: impl Into<String>,
        identifier: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::ResourceError {
            resource_type: resource_type.into(),
            identifier: identifier.into(),
            reason: reason.into(),
        }
    }

    /// Creates a capability error.
    #[must_use]
    pub fn capability_error(
        capability: impl Into<String>,
        value: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::CapabilityError {
            capability: capability.into(),
            value: value.into(),
            reason: reason.into(),
        }
    }

    /// Creates an operation failed error.
    #[must_use]
    pub fn operation_failed(operation: impl Into<String>, message: impl Into<String>) -> Self {
        Self::OperationFailed {
            operation: operation.into(),
            message: message.into(),
        }
    }

    /// Returns the error category for grouping similar errors.
    #[must_use]
    pub fn category(&self) -> &'static str {
        match self {
            Self::ConnectionFailed { .. } => "connection",
            Self::WebDriverError { .. } => "webdriver",
            Self::InvalidSession { .. } => "session",
            Self::Timeout { .. } => "timeout",
            Self::PoolExhausted { .. } => "pool",
            Self::Configuration { .. } => "config",
            Self::ProcessError { .. } => "process",
            Self::ElementError { .. } => "element",
            Self::NavigationError { .. } => "navigation",
            Self::ScriptError { .. } => "script",
            Self::ResourceError { .. } => "resource",
            Self::CapabilityError { .. } => "capability",
            Self::OperationFailed { .. } => "operation",
        }
    }

    /// Returns whether this error type is generally retryable.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::ConnectionFailed { .. } => true,
            Self::WebDriverError { error, .. } => is_webdriver_error_retryable(error.as_ref()),
            Self::InvalidSession { .. } => false, // Usually requires new session
            Self::Timeout { .. } => true,
            Self::PoolExhausted { .. } => true,
            Self::Configuration { .. } => false,
            Self::ProcessError { .. } => false,
            Self::ElementError { .. } => false, // Usually a logic error
            Self::NavigationError { error_type, .. } => matches!(
                error_type,
                NavigationErrorType::NetworkError
                    | NavigationErrorType::Timeout
                    | NavigationErrorType::HttpError(500..=599)
            ),
            Self::ScriptError { .. } => false,
            Self::ResourceError { .. } => true,
            Self::CapabilityError { .. } => false,
            Self::OperationFailed { .. } => true,
        }
    }

    /// Returns the error kind for Spire error categorization.
    pub fn error_kind(&self) -> ErrorKind {
        match self {
            Self::ConnectionFailed { .. } => ErrorKind::Http,
            Self::WebDriverError { .. } => ErrorKind::Backend,
            Self::InvalidSession { .. } => ErrorKind::Backend,
            Self::Timeout { .. } => ErrorKind::Timeout,
            Self::PoolExhausted { .. } => ErrorKind::Backend,
            Self::Configuration { .. } => ErrorKind::Backend,
            Self::ProcessError { .. } => ErrorKind::Backend,
            Self::ElementError { .. } => ErrorKind::Worker,
            Self::NavigationError { .. } => ErrorKind::Http,
            Self::ScriptError { .. } => ErrorKind::Worker,
            Self::ResourceError { .. } => ErrorKind::Io,
            Self::CapabilityError { .. } => ErrorKind::Backend,
            Self::OperationFailed { .. } => ErrorKind::Backend,
        }
    }
}

impl From<BrowserError> for Error {
    fn from(err: BrowserError) -> Self {
        Error::new(err.error_kind(), err.to_string())
    }
}

impl From<WebDriverError> for BrowserError {
    fn from(error: WebDriverError) -> Self {
        Self::WebDriverError {
            error: Box::new(error),
            context: None,
        }
    }
}

impl From<NavigationErrorType> for BrowserError {
    fn from(error_type: NavigationErrorType) -> Self {
        Self::NavigationError {
            url: "unknown".to_string(),
            error_type,
            details: None,
        }
    }
}

/// Determines if a WebDriver error is retryable.
fn is_webdriver_error_retryable(_error: &WebDriverError) -> bool {
    // For now, be conservative and consider most WebDriver errors as non-retryable
    // This can be refined based on specific error types as needed
    false
}

/// Type alias for `Result<T, BrowserError>` for more ergonomic error handling.
///
/// # Example
///
/// ```no_run
/// use spire_thirtyfour::{BrowserResult, BrowserConnection};
///
/// async fn navigate_to_page(connection: &BrowserConnection, url: &str) -> BrowserResult<()> {
///     // connection.navigate(url).await
///     Ok(())
/// }
/// ```
pub type BrowserResult<T> = Result<T, BrowserError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_error_display() {
        let error = BrowserError::timeout("page_load", 30);
        assert_eq!(
            error.to_string(),
            "Operation 'page_load' timed out after 30 seconds"
        );

        let nav_error = BrowserError::navigation_error(
            "https://example.com",
            NavigationErrorType::HttpError(404),
            Some("Page not found".to_string()),
        );
        assert!(nav_error.to_string().contains("Navigation to"));
        assert!(nav_error.to_string().contains("404"));
    }

    #[test]
    fn browser_error_category() {
        assert_eq!(BrowserError::timeout("test", 30).category(), "timeout");
        assert_eq!(
            BrowserError::connection_failed("http://localhost:4444", "Connection refused")
                .category(),
            "connection"
        );
        assert_eq!(
            BrowserError::configuration("Invalid config", None).category(),
            "config"
        );
    }

    #[test]
    fn browser_error_retryable() {
        assert!(
            BrowserError::connection_failed("http://localhost:4444", "Connection refused")
                .is_retryable()
        );

        assert!(!BrowserError::configuration("Invalid config", None).is_retryable());

        assert!(BrowserError::timeout("navigation", 30).is_retryable());

        let nav_error = BrowserError::navigation_error(
            "https://example.com",
            NavigationErrorType::HttpError(500),
            None,
        );
        assert!(nav_error.is_retryable());

        let nav_error_client = BrowserError::navigation_error(
            "https://example.com",
            NavigationErrorType::HttpError(404),
            None,
        );
        assert!(!nav_error_client.is_retryable());
    }

    #[test]
    fn navigation_error_type_display() {
        assert_eq!(NavigationErrorType::InvalidUrl.to_string(), "Invalid URL");
        assert_eq!(
            NavigationErrorType::HttpError(404).to_string(),
            "HTTP error 404"
        );
        assert_eq!(
            NavigationErrorType::NetworkError.to_string(),
            "Network error"
        );
    }

    #[test]
    fn browser_error_to_spire_error() {
        let browser_err = BrowserError::timeout("test", 30);
        let spire_err: Error = browser_err.into();
        assert_eq!(spire_err.kind(), ErrorKind::Timeout);
    }
}
