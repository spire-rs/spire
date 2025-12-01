use deadpool::managed::Pool;
use derive_builder::Builder;
use derive_more::Display;
use spire_core::{Error, ErrorKind, Result};

use crate::config::{BrowserType, PoolConfig, WebDriverConfig};
use crate::pool::BrowserPool;
use crate::pool::manager::BrowserManagerBuilder;

/// Configuration for a browser pool that will be built.
#[derive(Debug, Clone, Builder)]
#[builder(
    name = "BrowserPoolConfigBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(validate = "Self::validate_config")
)]
pub struct BrowserPoolConfig {
    /// WebDriver configurations
    #[builder(default = "Vec::new()")]
    pub configs: Vec<WebDriverConfig>,

    /// Pool configuration settings
    #[builder(default = "None")]
    pub pool_config: Option<PoolConfig>,

    /// Whether to enable health checks
    #[builder(default = "Some(true)")]
    pub health_checks: Option<bool>,

    /// Maximum retry attempts for connection creation
    #[builder(default = "Some(3)")]
    pub max_retry_attempts: Option<usize>,

    /// Whether to validate configurations before building
    #[builder(default = "true")]
    pub validate_configs: bool,
}

impl From<spire_core::Error> for BrowserPoolConfigBuilderError {
    fn from(err: spire_core::Error) -> Self {
        Self::ValidationError(err.to_string())
    }
}

impl From<BrowserPoolConfigBuilderError> for spire_core::Error {
    fn from(err: BrowserPoolConfigBuilderError) -> Self {
        Error::new(ErrorKind::Backend, err.to_string())
    }
}

impl BrowserPoolConfigBuilder {
    fn validate_config(&self) -> Result<()> {
        // Validate we have at least one configuration
        let binding = Vec::new();
        let configs = self.configs.as_ref().unwrap_or(&binding);
        if configs.is_empty() {
            return Err(Error::new(
                ErrorKind::Backend,
                "At least one WebDriver configuration is required",
            ));
        }

        // Validate all configurations if validation is enabled
        let validate = self.validate_configs.unwrap_or(true);
        if validate {
            for config in configs {
                config.validate()?;
            }
        }

        // Validate pool configuration if provided
        if let Some(Some(pool_config)) = self.pool_config.as_ref() {
            pool_config.validate()?;
        }

        // Validate optional settings
        if let Some(Some(max_retries)) = self.max_retry_attempts.as_ref()
            && *max_retries == 0
        {
            return Err(Error::new(
                ErrorKind::Backend,
                "Max retry attempts must be greater than zero",
            ));
        }

        Ok(())
    }
}

impl BrowserPoolConfig {
    /// Creates a new builder for browser pool configuration.
    pub fn builder() -> BrowserPoolConfigBuilder {
        BrowserPoolConfigBuilder::default()
    }

    /// Builds the browser pool from this configuration.
    pub fn build_pool(self) -> Result<BrowserPool> {
        // Build the browser manager
        let mut manager_builder = BrowserManagerBuilder::new();

        // Add all configurations
        let configs_len = self.configs.len();
        for config in &self.configs {
            manager_builder = manager_builder.add_config(config.clone());
        }

        // Apply pool configuration
        let pool_config = self.pool_config.unwrap_or_else(|| {
            // Create default pool config with size based on number of configurations
            PoolConfig::new().with_max_size(configs_len.max(1))
        });

        manager_builder = manager_builder.pool_config(pool_config.clone());

        // Apply optional settings
        if let Some(health_checks) = self.health_checks {
            manager_builder = manager_builder.health_checks(health_checks);
        }

        if let Some(max_retries) = self.max_retry_attempts {
            manager_builder = manager_builder.max_retry_attempts(max_retries);
        }

        // Build the manager
        let manager = manager_builder.build()?;

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

        Ok(BrowserPool::new(pool))
    }
}

/// Builder for configuring and creating a [`BrowserPool`].
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
/// use spire_thirtyfour::BrowserPool;
///
/// let pool = BrowserPool::builder()
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
/// use spire_thirtyfour::{BrowserPool, config::*};
/// use std::time::Duration;
///
/// let chrome_config = WebDriverConfig::builder()
///     .with_url("http://localhost:4444")
///     .with_browser(BrowserType::chrome())
///     .with_connect_timeout(Duration::from_secs(30))
///     .build()?;
///
/// let pool = BrowserPool::builder()
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
#[display(
    fmt = "BrowserBuilder(configs: {}, health_checks: {:?})",
    "self.config_builder.configs.as_ref().map(|c| c.len()).unwrap_or(0)",
    "self.config_builder.health_checks"
)]
pub struct BrowserBuilder {
    config_builder: BrowserPoolConfigBuilder,
}

impl BrowserBuilder {
    /// Creates a new [`BrowserBuilder`].
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_thirtyfour::BrowserPool;
    ///
    /// let builder = BrowserPool::builder();
    /// ```
    #[inline]
    pub fn new() -> Self {
        Self {
            config_builder: BrowserPoolConfigBuilder::default(),
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
    /// use spire_thirtyfour::BrowserPool;
    ///
    /// let pool = BrowserPool::builder()
    ///     .with_unmanaged("http://localhost:4444")
    ///     .with_unmanaged("http://localhost:4445")
    ///     .build()?;
    /// ```
    pub fn with_unmanaged(mut self, addr: impl AsRef<str>) -> Self {
        let config = WebDriverConfig::new(addr.as_ref()).with_browser(BrowserType::Chrome);

        // Get current configs and add new one
        let mut configs = self.config_builder.configs.take().unwrap_or_default();
        configs.push(config);
        self.config_builder.configs = Some(configs);

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
    /// use spire_thirtyfour::{BrowserPool, BrowserType};
    ///
    /// let pool = BrowserPool::builder()
    ///     .with_unmanaged_browser("http://localhost:4444", BrowserType::Chrome)
    ///     .with_unmanaged_browser("http://localhost:4445", BrowserType::Firefox)
    ///     .build()?;
    /// ```
    pub fn with_unmanaged_browser(mut self, addr: impl AsRef<str>, browser: BrowserType) -> Self {
        let config = WebDriverConfig::new(addr.as_ref()).with_browser(browser);

        // Get current configs and add new one
        let mut configs = self.config_builder.configs.take().unwrap_or_default();
        configs.push(config);
        self.config_builder.configs = Some(configs);

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
    /// use spire_thirtyfour::{BrowserPool, config::*};
    /// use std::time::Duration;
    ///
    /// let config = WebDriverConfig::builder()
    ///     .with_url("http://localhost:4444")
    ///     .with_browser(BrowserType::chrome())
    ///     .with_connect_timeout(Duration::from_secs(30))
    ///     .build()?;
    ///
    /// let pool = BrowserPool::builder()
    ///     .with_config(config)?
    ///     .build()?;
    /// ```
    pub fn with_config(mut self, config: WebDriverConfig) -> Result<Self> {
        // Validate if needed
        if self.config_builder.validate_configs.unwrap_or(true) {
            config.validate()?;
        }

        // Get current configs and add new one
        let mut configs = self.config_builder.configs.take().unwrap_or_default();
        configs.push(config);
        self.config_builder.configs = Some(configs);

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
        // Validate if needed
        if self.config_builder.validate_configs.unwrap_or(true) {
            for config in &new_configs {
                config.validate()?;
            }
        }

        // Get current configs and extend
        let mut configs = self.config_builder.configs.take().unwrap_or_default();
        configs.extend(new_configs);
        self.config_builder.configs = Some(configs);

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
        self.config_builder = self.config_builder.with_pool_config(config);
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
        self.config_builder = self.config_builder.with_health_checks(enabled);
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
        self.config_builder = self.config_builder.with_max_retry_attempts(attempts);
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
    pub fn without_config_validation(mut self) -> Self {
        self.config_builder = self.config_builder.with_validate_configs(false);
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
            "Managed browser processes are not yet implemented. Use with_unmanaged() instead.",
        ))
    }

    /// Constructs the [`BrowserPool`] from this builder.
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
    pub fn build(self) -> Result<BrowserPool> {
        let config = self.config_builder.build().map_err(Error::from)?;
        config.build_pool()
    }

    /// Returns the number of configurations added to this builder.
    pub fn config_count(&self) -> usize {
        self.config_builder
            .configs
            .as_ref()
            .map(|c| c.len())
            .unwrap_or(0)
    }

    /// Returns whether health checks are enabled.
    pub fn health_checks_enabled(&self) -> Option<bool> {
        self.config_builder.health_checks.flatten()
    }

    /// Returns the configured maximum retry attempts.
    pub fn max_retry_attempts(&self) -> Option<usize> {
        self.config_builder.max_retry_attempts.flatten()
    }
}

/// Error type for builder-related errors.
#[derive(Debug, Display)]
pub enum BuilderError {
    #[display(fmt = "Configuration error: {}", message)]
    Config { message: String },

    #[display(fmt = "Validation error: {}", message)]
    Validation { message: String },

    #[display(fmt = "Build error: {}", message)]
    Build { message: String },
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
    fn browser_pool_config_builder() {
        let config = BrowserPoolConfig::builder()
            .with_configs(vec![
                WebDriverConfig::new("http://localhost:4444"),
                WebDriverConfig::new("http://localhost:4445"),
            ])
            .with_health_checks(true)
            .with_max_retry_attempts(5_usize)
            .build()
            .expect("Should build successfully");

        assert_eq!(config.configs.len(), 2);
        assert_eq!(config.health_checks, Some(true));
        assert_eq!(config.max_retry_attempts, Some(5));
    }

    #[test]
    fn pool_config_validation() {
        // Empty configs should fail
        let result = BrowserPoolConfig::builder().build();
        assert!(result.is_err());

        // Zero max retries should fail
        let result = BrowserPoolConfig::builder()
            .with_configs(vec![WebDriverConfig::new("http://localhost:4444")])
            .with_max_retry_attempts(0_usize)
            .build();
        assert!(result.is_err());
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
        assert!(display.contains("health_checks: Some(Some(true))"));
    }
}
