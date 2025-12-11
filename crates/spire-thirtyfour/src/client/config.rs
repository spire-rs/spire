//! `WebDriver` and pool configuration types.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use derive_builder::Builder;
use derive_more::Display;
use serde_json::Value;
use spire_core::{Error, ErrorKind, Result};

/// Configuration for a browser connection.
#[derive(Debug, Clone, Builder)]
#[builder(
    name = "BrowserConfigBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(validate = "Self::validate_config")
)]
pub struct BrowserConfig {
    /// The `WebDriver` server URL (e.g., <http://127.0.0.1:4444>)
    pub url: String,

    /// `WebDriver` capabilities
    #[builder(default = "HashMap::new()")]
    pub capabilities: HashMap<String, Value>,

    /// Connection timeout
    #[builder(default = "Duration::from_secs(30)")]
    pub connect_timeout: Duration,

    /// Request timeout for `WebDriver` commands
    #[builder(default = "Duration::from_secs(60)")]
    pub request_timeout: Duration,

    /// Whether this is a managed or unmanaged connection
    #[builder(default = "false")]
    pub managed: bool,
}

impl From<spire_core::Error> for BrowserConfigBuilderError {
    fn from(err: spire_core::Error) -> Self {
        Self::ValidationError(err.to_string())
    }
}

impl BrowserConfigBuilder {
    fn validate_config(&self) -> Result<()> {
        // Validate URL format
        if let Some(ref url) = self.url {
            if url.is_empty() {
                return Err(Error::new(
                    ErrorKind::Backend,
                    "WebDriver URL cannot be empty",
                ));
            }

            // Basic URL validation
            if !url.starts_with("http://") && !url.starts_with("https://") {
                return Err(Error::new(
                    ErrorKind::Backend,
                    format!("Invalid WebDriver URL format: {url}"),
                ));
            }
        }

        // Validate timeouts
        if let Some(ref timeout) = self.connect_timeout
            && timeout.is_zero()
        {
            return Err(Error::new(
                ErrorKind::Backend,
                "Connect timeout must be greater than zero",
            ));
        }

        if let Some(ref timeout) = self.request_timeout
            && timeout.is_zero()
        {
            return Err(Error::new(
                ErrorKind::Backend,
                "Request timeout must be greater than zero",
            ));
        }

        Ok(())
    }
}

impl BrowserConfig {
    /// Creates a new browser configuration with default values.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            capabilities: HashMap::new(),
            connect_timeout: Duration::from_secs(30),
            request_timeout: Duration::from_secs(60),
            managed: false,
        }
    }

    /// Creates a builder for browser configuration.
    #[must_use]
    pub fn builder() -> BrowserConfigBuilder {
        BrowserConfigBuilder::default()
    }

    /// Sets a `WebDriver` capability.
    #[must_use]
    pub fn with_capability(mut self, key: impl Into<String>, value: Value) -> Self {
        self.capabilities.insert(key.into(), value);
        self
    }

    /// Sets multiple `WebDriver` capabilities.
    #[must_use]
    pub fn with_capabilities(mut self, capabilities: HashMap<String, Value>) -> Self {
        self.capabilities.extend(capabilities);
        self
    }

    /// Sets the connection timeout for this configuration.
    #[must_use]
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Marks this connection as managed (browser process will be spawned).
    #[must_use]
    pub fn managed(mut self) -> Self {
        self.managed = true;
        self
    }

    /// Marks this connection as unmanaged (connects to existing `WebDriver` server).
    #[must_use]
    pub fn unmanaged(mut self) -> Self {
        self.managed = false;
        self
    }

    /// Validates the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid (empty URL, invalid URL format, zero timeouts).
    pub fn validate(&self) -> Result<()> {
        // Validate URL format
        if self.url.is_empty() {
            return Err(Error::new(
                ErrorKind::Backend,
                "WebDriver URL cannot be empty",
            ));
        }

        // Basic URL validation
        if !self.url.starts_with("http://") && !self.url.starts_with("https://") {
            return Err(Error::new(
                ErrorKind::Backend,
                format!("Invalid WebDriver URL format: {}", self.url),
            ));
        }

        // Validate timeouts
        if self.connect_timeout.is_zero() {
            return Err(Error::new(
                ErrorKind::Backend,
                "Connect timeout must be greater than zero",
            ));
        }

        if self.request_timeout.is_zero() {
            return Err(Error::new(
                ErrorKind::Backend,
                "Request timeout must be greater than zero",
            ));
        }

        Ok(())
    }

    /// Returns the base `WebDriver` URL without path components.
    #[must_use]
    pub fn base_url(&self) -> &str {
        // Remove trailing slashes and paths if present
        self.url.trim_end_matches('/')
    }
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self::new("http://127.0.0.1:4444")
    }
}

impl fmt::Display for BrowserConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Browser({}, managed: {})", self.url, self.managed)
    }
}

/// Wrapper type for easier error handling in builders
#[derive(Debug, Display)]
pub enum ConfigError {
    #[display("Invalid URL: {}", message)]
    /// Invalid URL provided in configuration
    InvalidUrl {
        /// Error message describing the invalid URL
        message: String,
    },

    #[display("Invalid timeout: {}", message)]
    /// Invalid timeout value provided in configuration
    InvalidTimeout {
        /// Error message describing the invalid timeout
        message: String,
    },

    #[display("Invalid pool configuration: {}", message)]
    /// Invalid pool configuration provided
    InvalidPool {
        /// Error message describing the invalid pool configuration
        message: String,
    },

    #[display("Validation error: {}", message)]
    /// Configuration validation failed
    Validation {
        /// Error message describing the validation failure
        message: String,
    },
}

impl std::error::Error for ConfigError {}

impl From<ConfigError> for Error {
    fn from(err: ConfigError) -> Self {
        Error::new(ErrorKind::Backend, err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn browser_config_builder() {
        let config = BrowserConfig::builder()
            .with_url("http://localhost:4444")
            .with_connect_timeout(Duration::from_secs(10))
            .with_managed(true)
            .build()
            .expect("Should build successfully");

        assert_eq!(config.url, "http://localhost:4444");
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert!(config.managed);
    }

    #[test]
    fn browser_config_builder_validation() {
        // Empty URL should fail validation
        let result = BrowserConfig::builder().with_url("").build();
        assert!(result.is_err());

        // Invalid URL should fail validation
        let result = BrowserConfig::builder().with_url("invalid-url").build();
        assert!(result.is_err());

        // Zero timeout should fail validation
        let result = BrowserConfig::builder()
            .with_url("http://localhost:4444")
            .with_connect_timeout(Duration::ZERO)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn browser_config_new() {
        let config = BrowserConfig::new("http://localhost:4444");
        assert_eq!(config.url, "http://localhost:4444");
        assert!(!config.managed);
    }

    #[test]
    fn browser_config_validation() {
        // Valid config should pass
        let config = BrowserConfig::new("http://localhost:4444");
        assert!(config.validate().is_ok());

        // Empty URL should fail
        let config = BrowserConfig {
            url: String::new(),
            ..Default::default()
        };
        assert!(config.validate().is_err());

        // Invalid URL format should fail
        let config = BrowserConfig::new("localhost:4444");
        assert!(config.validate().is_err());
    }

    #[test]
    fn browser_config_capabilities() {
        let config = BrowserConfig::new("http://localhost:4444")
            .with_capability("browserName", json!("firefox"))
            .with_connect_timeout(Duration::from_secs(10))
            .managed();

        assert!(config.managed);
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert!(config.capabilities.contains_key("browserName"));
    }

    #[test]
    fn config_error_display() {
        let error = ConfigError::InvalidUrl {
            message: "Empty URL".to_string(),
        };
        assert_eq!(error.to_string(), "Invalid URL: Empty URL");

        let error = ConfigError::InvalidTimeout {
            message: "Timeout is zero".to_string(),
        };
        assert_eq!(error.to_string(), "Invalid timeout: Timeout is zero");
    }
}
