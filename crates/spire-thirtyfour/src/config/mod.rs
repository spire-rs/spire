//! Configuration types for WebDriver connections.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use derive_builder::Builder;
use derive_more::Display;
use serde_json::Value;
use spire_core::{Error, ErrorKind, Result};

pub mod browser;
pub mod capabilities;

pub use browser::BrowserType;

/// Configuration for a WebDriver endpoint.
#[derive(Debug, Clone, Builder)]
#[builder(
    name = "WebDriverConfigBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(validate = "Self::validate_config")
)]
pub struct WebDriverConfig {
    /// The WebDriver server URL (e.g., "http://127.0.0.1:4444")
    pub url: String,

    /// Browser type and version requirements
    #[builder(default = "BrowserType::Chrome")]
    pub browser: BrowserType,

    /// WebDriver capabilities
    #[builder(default = "HashMap::new()")]
    pub capabilities: HashMap<String, Value>,

    /// Connection timeout
    #[builder(default = "Duration::from_secs(30)")]
    pub connect_timeout: Duration,

    /// Request timeout for WebDriver commands
    #[builder(default = "Duration::from_secs(60)")]
    pub request_timeout: Duration,

    /// Whether this is a managed or unmanaged connection
    #[builder(default = "false")]
    pub managed: bool,
}

impl From<spire_core::Error> for WebDriverConfigBuilderError {
    fn from(err: spire_core::Error) -> Self {
        Self::ValidationError(err.to_string())
    }
}

impl WebDriverConfigBuilder {
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
                    format!("Invalid WebDriver URL format: {}", url),
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

impl WebDriverConfig {
    /// Creates a new WebDriver configuration with default values.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            browser: BrowserType::Chrome,
            capabilities: HashMap::new(),
            connect_timeout: Duration::from_secs(30),
            request_timeout: Duration::from_secs(60),
            managed: false,
        }
    }

    /// Creates a builder for WebDriver configuration.
    pub fn builder() -> WebDriverConfigBuilder {
        WebDriverConfigBuilder::default()
    }

    /// Sets a WebDriver capability.
    pub fn with_capability(mut self, key: impl Into<String>, value: Value) -> Self {
        self.capabilities.insert(key.into(), value);
        self
    }

    /// Sets multiple WebDriver capabilities.
    pub fn with_capabilities(mut self, capabilities: HashMap<String, Value>) -> Self {
        self.capabilities.extend(capabilities);
        self
    }

    /// Sets the browser type for this configuration.
    pub fn with_browser(mut self, browser: BrowserType) -> Self {
        self.browser = browser;
        self
    }

    /// Sets the connection timeout for this configuration.
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Marks this connection as managed (browser process will be spawned).
    pub fn managed(mut self) -> Self {
        self.managed = true;
        self
    }

    /// Marks this connection as unmanaged (connects to existing WebDriver server).
    pub fn unmanaged(mut self) -> Self {
        self.managed = false;
        self
    }

    /// Validates the configuration.
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

    /// Returns the base WebDriver URL without path components.
    pub fn base_url(&self) -> &str {
        // Remove trailing slashes and paths if present
        self.url.trim_end_matches('/')
    }
}

impl Default for WebDriverConfig {
    fn default() -> Self {
        Self::new("http://127.0.0.1:4444")
    }
}

impl fmt::Display for WebDriverConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WebDriver({}, {}, managed: {})",
            self.url, self.browser, self.managed
        )
    }
}

/// Pool configuration for browser connections.
#[derive(Debug, Clone, Builder)]
#[builder(
    name = "PoolConfigBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(validate = "Self::validate_config")
)]
pub struct PoolConfig {
    /// Maximum number of browser instances in the pool
    #[builder(default = "10")]
    pub max_size: usize,

    /// Minimum number of browser instances to keep alive
    #[builder(default = "1")]
    pub min_size: usize,

    /// Timeout for acquiring a browser from the pool
    #[builder(default = "Duration::from_secs(30)")]
    pub acquire_timeout: Duration,

    /// Maximum lifetime of a browser instance
    #[builder(default = "Some(Duration::from_secs(3600))")]
    pub max_lifetime: Option<Duration>,

    /// Maximum idle time before recycling a browser
    #[builder(default = "Some(Duration::from_secs(300))")]
    pub max_idle_time: Option<Duration>,

    /// Health check interval for browser instances
    #[builder(default = "Duration::from_secs(60)")]
    pub health_check_interval: Duration,
}

impl From<spire_core::Error> for PoolConfigBuilderError {
    fn from(err: spire_core::Error) -> Self {
        Self::ValidationError(err.to_string())
    }
}

impl PoolConfigBuilder {
    fn validate_config(&self) -> Result<()> {
        let max_size = self.max_size.unwrap_or(10);
        let min_size = self.min_size.unwrap_or(1);

        if max_size == 0 {
            return Err(Error::new(
                ErrorKind::Backend,
                "Pool max_size must be greater than zero",
            ));
        }

        if min_size > max_size {
            return Err(Error::new(
                ErrorKind::Backend,
                "Pool min_size cannot be greater than max_size",
            ));
        }

        if let Some(ref timeout) = self.acquire_timeout
            && timeout.is_zero()
        {
            return Err(Error::new(
                ErrorKind::Backend,
                "Acquire timeout must be greater than zero",
            ));
        }

        Ok(())
    }
}

impl PoolConfig {
    /// Creates a new pool configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a builder for pool configuration.
    pub fn builder() -> PoolConfigBuilder {
        PoolConfigBuilder::default()
    }

    /// Validates the pool configuration.
    pub fn validate(&self) -> Result<()> {
        if self.max_size == 0 {
            return Err(Error::new(
                ErrorKind::Backend,
                "Pool max_size must be greater than zero",
            ));
        }

        if self.min_size > self.max_size {
            return Err(Error::new(
                ErrorKind::Backend,
                "Pool min_size cannot be greater than max_size",
            ));
        }

        if self.acquire_timeout.is_zero() {
            return Err(Error::new(
                ErrorKind::Backend,
                "Acquire timeout must be greater than zero",
            ));
        }

        Ok(())
    }

    /// Sets the maximum pool size.
    pub fn with_max_size(mut self, max_size: usize) -> Self {
        self.max_size = max_size;
        self
    }

    /// Sets the minimum pool size.
    pub fn with_min_size(mut self, min_size: usize) -> Self {
        self.min_size = min_size;
        self
    }

    /// Sets the acquire timeout.
    pub fn with_acquire_timeout(mut self, timeout: Duration) -> Self {
        self.acquire_timeout = timeout;
        self
    }

    /// Sets the maximum connection lifetime.
    pub fn with_max_lifetime(mut self, lifetime: Duration) -> Self {
        self.max_lifetime = Some(lifetime);
        self
    }

    /// Sets the maximum idle time.
    pub fn with_max_idle_time(mut self, idle_time: Duration) -> Self {
        self.max_idle_time = Some(idle_time);
        self
    }

    /// Sets the health check interval.
    pub fn with_health_check_interval(mut self, interval: Duration) -> Self {
        self.health_check_interval = interval;
        self
    }
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_size: 10,
            min_size: 1,
            acquire_timeout: Duration::from_secs(30),
            max_lifetime: Some(Duration::from_secs(3600)), // 1 hour
            max_idle_time: Some(Duration::from_secs(300)), // 5 minutes
            health_check_interval: Duration::from_secs(60), // 1 minute
        }
    }
}

/// Wrapper type for easier error handling in builders
#[derive(Debug, Display)]
pub enum ConfigError {
    #[display(fmt = "Invalid URL: {}", message)]
    /// Invalid URL provided in configuration
    InvalidUrl {
        /// Error message describing the invalid URL
        message: String,
    },

    #[display(fmt = "Invalid timeout: {}", message)]
    /// Invalid timeout value provided in configuration
    InvalidTimeout {
        /// Error message describing the invalid timeout
        message: String,
    },

    #[display(fmt = "Invalid pool configuration: {}", message)]
    /// Invalid pool configuration provided
    InvalidPool {
        /// Error message describing the invalid pool configuration
        message: String,
    },

    #[display(fmt = "Validation error: {}", message)]
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
    fn webdriver_config_builder() {
        let config = WebDriverConfig::builder()
            .with_url("http://localhost:4444")
            .with_browser(BrowserType::Firefox)
            .with_connect_timeout(Duration::from_secs(10))
            .with_managed(true)
            .build()
            .expect("Should build successfully");

        assert_eq!(config.url, "http://localhost:4444");
        assert!(matches!(config.browser, BrowserType::Firefox));
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert!(config.managed);
    }

    #[test]
    fn webdriver_config_builder_validation() {
        // Empty URL should fail validation
        let result = WebDriverConfig::builder().with_url("").build();
        assert!(result.is_err());

        // Invalid URL should fail validation
        let result = WebDriverConfig::builder().with_url("invalid-url").build();
        assert!(result.is_err());

        // Zero timeout should fail validation
        let result = WebDriverConfig::builder()
            .with_url("http://localhost:4444")
            .with_connect_timeout(Duration::ZERO)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn webdriver_config_new() {
        let config = WebDriverConfig::new("http://localhost:4444");
        assert_eq!(config.url, "http://localhost:4444");
        assert!(matches!(config.browser, BrowserType::Chrome));
        assert!(!config.managed);
    }

    #[test]
    fn webdriver_config_validation() {
        // Valid config should pass
        let config = WebDriverConfig::new("http://localhost:4444");
        assert!(config.validate().is_ok());

        // Empty URL should fail
        let config = WebDriverConfig {
            url: String::new(),
            ..Default::default()
        };
        assert!(config.validate().is_err());

        // Invalid URL format should fail
        let config = WebDriverConfig::new("localhost:4444");
        assert!(config.validate().is_err());
    }

    #[test]
    fn webdriver_config_capabilities() {
        let config = WebDriverConfig::new("http://localhost:4444")
            .with_capability("browserName", json!("firefox"))
            .with_connect_timeout(Duration::from_secs(10))
            .managed();

        assert!(config.managed);
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert!(config.capabilities.contains_key("browserName"));
    }

    #[test]
    fn pool_config_builder() {
        let config = PoolConfig::builder()
            .with_max_size(20_usize)
            .with_min_size(5_usize)
            .with_acquire_timeout(Duration::from_secs(45))
            .with_max_lifetime(Duration::from_secs(7200))
            .build()
            .expect("Should build successfully");

        assert_eq!(config.max_size, 20);
        assert_eq!(config.min_size, 5);
        assert_eq!(config.acquire_timeout, Duration::from_secs(45));
        assert_eq!(config.max_lifetime, Some(Duration::from_secs(7200)));
    }

    #[test]
    fn pool_config_builder_validation() {
        // Zero max_size should fail validation
        let result = PoolConfig::builder().with_max_size(0_usize).build();
        assert!(result.is_err());

        // min_size > max_size should fail validation
        let result = PoolConfig::builder()
            .with_max_size(5_usize)
            .with_min_size(10_usize)
            .build();
        assert!(result.is_err());

        // Zero acquire timeout should fail validation
        let result = PoolConfig::builder()
            .with_acquire_timeout(Duration::ZERO)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn pool_config_validation() {
        // Valid config should pass
        let config = PoolConfig::default();
        assert!(config.validate().is_ok());

        // Zero max_size should fail
        let config = PoolConfig {
            max_size: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());

        // min_size > max_size should fail
        let config = PoolConfig {
            min_size: 5,
            max_size: 3,
            ..Default::default()
        };
        assert!(config.validate().is_err());
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
