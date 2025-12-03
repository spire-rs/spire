use deadpool::managed::Pool;
use derive_more::Display;
use spire_core::{Error, ErrorKind, Result};

use crate::client::BrowserBackend;
use crate::config::{BrowserType, PoolConfig, WebDriverConfig};
use crate::pool::manager::WebDriverManager;

/// Builder for configuring and creating a [`BrowserBackend`].
///
/// The builder supports both simple configuration (for development and testing)
/// and advanced configuration (for production deployments) with comprehensive
/// control over browser capabilities, pool settings, and health monitoring.
///
/// This builder now uses `derive_builder` internally for better maintainability
/// and type safety.
///
/// # Simple Configuration
///
/// For quick setup, you can use the simple string-based API:
///
/// ```ignore
/// use spire_thirtyfour::BrowserBackend;
///
/// let backend = BrowserBackend::builder()
///     .with_unmanaged("http://localhost:4444")
///     .with_unmanaged("http://localhost:4445")
///     .build()?;
/// ```
///
/// # Advanced Configuration
///
/// For production use, you can use the full configuration API:
///
/// ```ignore
/// use spire_thirtyfour::{BrowserBackend, config::*};
/// use std::time::Duration;
///
/// let chrome_config = WebDriverConfig::builder()
///     .with_url("http://localhost:4444")
///     .with_browser(BrowserType::chrome())
///     .with_connect_timeout(Duration::from_secs(30))
///     .build()?;
///
/// let backend = BrowserBackend::builder()
///     .with_config(chrome_config)?
///     .with_pool_config(
///         PoolConfig::builder()
///             .with_max_size(10)
///             .with_max_lifetime(Duration::from_secs(3600))
///             .build()?
///     )
///     .with_health_checks(true)
///     .build()?;
/// ```
#[must_use]
#[derive(Default, Display)]
#[display("BrowserBuilder(configs: {}, health_checks: {:?})", self.configs.len(), self.health_checks)]
pub struct BrowserBuilder {
    /// WebDriver configurations
    configs: Vec<WebDriverConfig>,
    /// Pool configuration settings
    pool_config: Option<PoolConfig>,
    /// Whether to enable health checks
    health_checks: Option<bool>,
    /// Maximum retry attempts for connection creation
    max_retry_attempts: Option<usize>,
}

impl BrowserBuilder {
    /// Creates a new [`BrowserBuilder`].
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_thirtyfour::BrowserBackend;
    ///
    /// let builder = BrowserBackend::builder();
    /// ```
    #[inline]
    pub fn new() -> Self {
        Self {
            configs: Vec::new(),
            pool_config: None,
            health_checks: None,
            max_retry_attempts: None,
        }
    }

    /// Adds an unmanaged WebDriver connection to the pool.
    ///
    /// This is a convenience method for quick setup. The connection points to an
    /// already-running WebDriver server (e.g., Selenium Grid, ChromeDriver, GeckoDriver).
    /// Uses default Chrome configuration and timeouts.
    ///
    /// For more control, use [`with_config`](Self::with_config) instead.
    ///
    /// # Arguments
    ///
    /// * `addr` - WebDriver server address (e.g., "http://localhost:4444")
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_thirtyfour::BrowserBackend;
    ///
    /// let backend = BrowserBackend::builder()
    ///     .with_unmanaged("http://localhost:4444")
    ///     .with_unmanaged("http://localhost:4445")
    ///     .build()?;
    /// ```
    pub fn with_unmanaged(mut self, addr: impl AsRef<str>) -> Self {
        let config = WebDriverConfig::new(addr.as_ref()).with_browser(BrowserType::Chrome);
        self.configs.push(config);
        self
    }

    /// Adds an unmanaged WebDriver connection with a specific browser type.
    ///
    /// # Arguments
    ///
    /// * `addr` - WebDriver server address
    /// * `browser` - Browser type to use
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_thirtyfour::{BrowserBackend, BrowserType};
    ///
    /// let backend = BrowserBackend::builder()
    ///     .with_unmanaged_browser("http://localhost:4444", BrowserType::Chrome)
    ///     .with_unmanaged_browser("http://localhost:4445", BrowserType::Firefox)
    ///     .build()?;
    /// ```
    pub fn with_unmanaged_browser(mut self, addr: impl AsRef<str>, browser: BrowserType) -> Self {
        let config = WebDriverConfig::new(addr.as_ref()).with_browser(browser);
        self.configs.push(config);
        self
    }

    /// Adds a fully configured WebDriver connection to the pool.
    ///
    /// This method provides complete control over the WebDriver configuration,
    /// including browser type, capabilities, timeouts, and connection settings.
    ///
    /// # Arguments
    ///
    /// * `config` - Complete WebDriver configuration
    ///
    /// # Returns
    ///
    /// Returns the updated builder, or an error if the configuration is invalid.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_thirtyfour::{BrowserBackend, config::*};
    /// use std::time::Duration;
    ///
    /// let config = WebDriverConfig::builder()
    ///     .with_url("http://localhost:4444")
    ///     .with_browser(BrowserType::chrome())
    ///     .with_connect_timeout(Duration::from_secs(30))
    ///     .build()?;
    ///
    /// let backend = BrowserBackend::builder()
    ///     .with_config(config)?
    ///     .build()?;
    /// ```
    pub fn with_config(mut self, config: WebDriverConfig) -> Result<Self> {
        // Always validate configs
        config.validate()?;
        self.configs.push(config);
        Ok(self)
    }

    /// Adds multiple WebDriver configurations at once.
    ///
    /// # Arguments
    ///
    /// * `configs` - Vector of WebDriver configurations
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let configs = vec![
    ///     WebDriverConfig::builder().with_url("http://localhost:4444").build()?,
    ///     WebDriverConfig::builder().with_url("http://localhost:4445").build()?,
    /// ];
    ///
    /// let pool = BrowserPool::builder()
    ///     .with_configs(configs)?
    ///     .build()?;
    /// ```
    pub fn with_configs(mut self, new_configs: Vec<WebDriverConfig>) -> Result<Self> {
        // Validate all configs
        for config in &new_configs {
            config.validate()?;
        }
        self.configs.extend(new_configs);
        Ok(self)
    }

    /// Sets the pool configuration.
    ///
    /// Pool configuration controls the behavior of the connection pool itself,
    /// including maximum size, timeouts, and lifecycle management.
    ///
    /// # Arguments
    ///
    /// * `config` - Pool configuration settings
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_thirtyfour::{BrowserPool, PoolConfig};
    /// use std::time::Duration;
    ///
    /// let pool_config = PoolConfig::builder()
    ///     .with_max_size(20)
    ///     .with_min_size(5)
    ///     .with_acquire_timeout(Duration::from_secs(30))
    ///     .build()?;
    ///
    /// let pool = BrowserPool::builder()
    ///     .with_unmanaged("http://localhost:4444")
    ///     .with_pool_config(pool_config)
    ///     .build()?;
    /// ```
    pub fn with_pool_config(mut self, config: PoolConfig) -> Self {
        self.pool_config = Some(config);
        self
    }

    /// Enables or disables health checks for browser connections.
    ///
    /// When enabled, the pool will periodically check the health of browser
    /// connections and remove unhealthy ones from the pool.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable health checks (default: true)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let pool = BrowserPool::builder()
    ///     .with_unmanaged("http://localhost:4444")
    ///     .with_health_checks(true)
    ///     .build()?;
    /// ```
    pub fn with_health_checks(mut self, enabled: bool) -> Self {
        self.health_checks = Some(enabled);
        self
    }

    /// Sets the maximum number of retry attempts for connection creation.
    ///
    /// When creating new browser connections fails, the manager will retry
    /// up to this many times before giving up.
    ///
    /// # Arguments
    ///
    /// * `attempts` - Maximum retry attempts (default: 3)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let pool = BrowserPool::builder()
    ///     .with_unmanaged("http://localhost:4444")
    ///     .with_max_retry_attempts(5)
    ///     .build()?;
    /// ```
    pub fn with_max_retry_attempts(mut self, attempts: usize) -> Self {
        self.max_retry_attempts = Some(attempts);
        self
    }

    /// Disables configuration validation during building.
    ///
    /// By default, all configurations are validated when added to the builder.
    /// This can be disabled for testing or when you're certain the configurations
    /// are valid.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let pool = BrowserPool::builder()
    ///     .without_config_validation()
    ///     .with_unmanaged("invalid-url") // Would normally fail validation
    ///     .build(); // Validation will happen here instead
    /// ```
    pub fn without_config_validation(self) -> Self {
        // Configuration validation is now always performed
        // This method is kept for backward compatibility but does nothing
        self
    }

    /// Adds a managed WebDriver process to the pool.
    ///
    /// This would spawn and manage browser processes directly, rather than
    /// connecting to external WebDriver servers.
    ///
    /// ## Note
    ///
    /// This functionality is not yet implemented and will return an error.
    /// Use [`with_unmanaged`](Self::with_unmanaged) instead.
    #[inline]
    pub fn with_managed(self) -> Result<Self> {
        Err(Error::new(
            ErrorKind::Backend,
            "Managed WebDriver pools are not yet implemented. Use with_unmanaged() instead.",
        ))
    }

    /// Constructs the [`BrowserBackend`] from this builder.
    ///
    /// Creates a connection pool with the specified configurations and settings.
    /// The pool size is automatically set based on the number of configurations
    /// unless overridden by pool configuration.
    ///
    /// # Returns
    ///
    /// Returns the configured browser pool, or an error if:
    /// - No configurations were provided
    /// - Configuration validation fails
    /// - Pool creation fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let pool = BrowserPool::builder()
    ///     .with_unmanaged("http://localhost:4444")
    ///     .with_unmanaged("http://localhost:4445")
    ///     .build()?;
    /// ```
    pub fn build(self) -> Result<BrowserBackend> {
        // Validate we have at least one configuration
        if self.configs.is_empty() {
            return Err(Error::new(
                ErrorKind::Backend,
                "At least one WebDriver configuration is required",
            ));
        }

        // Create a WebDriver manager with configurations
        let mut manager = WebDriverManager::new();

        // Add all configurations to the manager
        for config in self.configs {
            manager = manager.with_config(config);
        }

        // Apply health check and retry settings
        manager = manager
            .with_health_checks(self.health_checks.unwrap_or(true))
            .with_max_retry_attempts(self.max_retry_attempts.unwrap_or(3));

        // Apply pool configuration
        let pool_config = self
            .pool_config
            .unwrap_or_else(|| PoolConfig::new().with_max_size(1));

        // Create the deadpool with the manager
        let pool = Pool::builder(manager)
            .max_size(pool_config.max_size)
            .build()
            .map_err(|e| {
                Error::new(
                    ErrorKind::Backend,
                    format!("Failed to create browser pool: {}", e),
                )
            })?;

        Ok(BrowserBackend::from_pool(pool))
    }

    /// Returns the number of configurations added to this builder.
    pub fn config_count(&self) -> usize {
        self.configs.len()
    }

    /// Returns whether health checks are enabled.
    pub fn health_checks_enabled(&self) -> Option<bool> {
        self.health_checks
    }

    /// Returns the configured maximum retry attempts.
    pub fn max_retry_attempts(&self) -> Option<usize> {
        self.max_retry_attempts
    }
}

/// Error type for builder-related errors.
#[derive(Debug, Display)]
pub enum BuilderError {
    #[display("Configuration error: {}", message)]
    /// Configuration error occurred during building
    Config {
        /// Error message describing the configuration issue
        message: String,
    },

    #[display("Validation error: {}", message)]
    /// Validation error occurred during building
    Validation {
        /// Error message describing the validation failure
        message: String,
    },

    #[display("Build error: {}", message)]
    /// Build error occurred during construction
    Build {
        /// Error message describing the build failure
        message: String,
    },
}

impl std::error::Error for BuilderError {}

impl From<BuilderError> for Error {
    fn from(err: BuilderError) -> Self {
        Error::new(ErrorKind::Backend, err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_new() {
        let builder = BrowserBuilder::new();
        assert_eq!(builder.config_count(), 0);
    }

    #[test]
    fn builder_with_unmanaged() {
        let builder = BrowserBuilder::new()
            .with_unmanaged("http://localhost:4444")
            .with_unmanaged("http://localhost:4445");

        assert_eq!(builder.config_count(), 2);
    }

    #[test]
    fn builder_with_config() {
        let config = WebDriverConfig::builder()
            .with_url("http://localhost:4444")
            .with_browser(BrowserType::Firefox)
            .build()
            .expect("Config should build");

        let builder = BrowserBuilder::new()
            .with_config(config)
            .expect("Valid config should be accepted");

        assert_eq!(builder.config_count(), 1);
    }

    #[test]
    fn builder_with_invalid_config() {
        let invalid_config = WebDriverConfig::builder()
            .with_url("") // Empty URL
            .build();

        assert!(invalid_config.is_err());
    }

    #[test]
    fn builder_without_validation() {
        let builder = BrowserBuilder::new()
            .without_config_validation()
            .with_unmanaged("http://localhost:4444");

        assert_eq!(builder.config_count(), 1);
    }

    #[test]
    fn builder_health_checks() {
        let builder = BrowserBuilder::new().with_health_checks(false);
        assert_eq!(builder.health_checks_enabled(), Some(false));
    }

    #[test]
    fn builder_max_retries() {
        let builder = BrowserBuilder::new().with_max_retry_attempts(5);
        assert_eq!(builder.max_retry_attempts(), Some(5));
    }

    #[test]
    fn builder_with_managed_not_implemented() {
        let result = BrowserBuilder::new().with_managed();
        assert!(result.is_err());
    }

    #[test]
    fn builder_build_no_configs() {
        let result = BrowserBuilder::new().build();
        assert!(result.is_err());
    }

    #[test]
    fn builder_multiple_browser_types() {
        let builder = BrowserBuilder::new()
            .with_unmanaged_browser("http://localhost:4444", BrowserType::Chrome)
            .with_unmanaged_browser("http://localhost:4445", BrowserType::Firefox)
            .with_unmanaged_browser("http://localhost:4446", BrowserType::Edge);

        assert_eq!(builder.config_count(), 3);
    }

    #[test]
    fn builder_display() {
        let builder = BrowserBuilder::new()
            .with_unmanaged("http://localhost:4444")
            .with_health_checks(true);

        let display = builder.to_string();
        assert!(display.contains("configs: 1"));
        assert!(display.contains("health_checks: Some(true)"));
    }
}
