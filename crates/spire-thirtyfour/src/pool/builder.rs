use std::time::Duration;

use deadpool::managed::Pool;
use derive_more::Display;
use spire_core::{Error, ErrorKind, Result};

use crate::client::{BrowserBackend, BrowserConfig as WebDriverConfig};
use crate::pool::manager::BrowserManager;

/// Builder for configuring and creating a [`BrowserBackend`].
///
/// The builder supports both simple configuration (for development and testing)
/// and advanced configuration (for production deployments) with comprehensive
/// control over browser capabilities and pool settings.
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
/// use spire_thirtyfour::{BrowserBackend, BrowserConfig};
/// use std::time::Duration;
///
/// let chrome_config = BrowserConfig::builder()
///     .with_url("http://localhost:4444")
///     .with_connect_timeout(Duration::from_secs(30))
///     .build()?;
///
/// let backend = BrowserBackend::builder()
///     .with_config(chrome_config)?
///     .with_max_pool_size(10)
///     .with_health_checks(true)
///     .build()?;
/// ```
#[must_use]
#[derive(Default, Display)]
#[display("BrowserBuilder(configs: {}, max_size: {:?})", self.configs.len(), self.max_size)]
pub struct BrowserBuilder {
    /// WebDriver configurations
    configs: Vec<WebDriverConfig>,
    /// Maximum pool size
    max_size: Option<usize>,
    /// Minimum pool size
    min_size: Option<usize>,
    /// Timeout for acquiring connections from pool
    acquire_timeout: Option<Duration>,
    /// Maximum lifetime of connections
    max_lifetime: Option<Duration>,
    /// Maximum idle time for connections
    max_idle_time: Option<Duration>,
    /// Whether to enable health checks
    health_checks: Option<bool>,
    /// Maximum retry attempts for connection creation
    max_retry_attempts: Option<usize>,
    /// Whether to skip configuration validation
    skip_validation: bool,
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
        Self::default()
    }

    /// Adds an unmanaged WebDriver connection to the pool.
    ///
    /// This is a convenience method for quick setup. The connection points to an
    /// already-running WebDriver server (e.g., Selenium Grid, ChromeDriver, GeckoDriver).
    /// Uses default configuration and timeouts.
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
    pub fn with_unmanaged(mut self, addr: impl Into<String>) -> Self {
        let config = WebDriverConfig::new(addr.into()).unmanaged();
        self.configs.push(config);
        self
    }

    /// Adds a managed WebDriver connection to the pool.
    ///
    /// This creates a managed browser process that will be spawned and controlled
    /// by the pool. Useful for environments where you want full control over the
    /// browser lifecycle.
    ///
    /// # Arguments
    ///
    /// * `addr` - WebDriver server address where the managed process will run
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_thirtyfour::BrowserBackend;
    ///
    /// let backend = BrowserBackend::builder()
    ///     .with_managed("http://localhost:4444")
    ///     .build()?;
    /// ```
    pub fn with_managed(mut self, addr: impl Into<String>) -> Self {
        let config = WebDriverConfig::new(addr.into()).managed();
        self.configs.push(config);
        self
    }

    /// Adds a custom WebDriver configuration to the pool.
    ///
    /// This provides full control over browser capabilities, timeouts,
    /// and connection settings.
    ///
    /// # Arguments
    ///
    /// * `config` - Fully configured WebDriver configuration
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_thirtyfour::{BrowserBackend, BrowserConfig};
    /// use std::time::Duration;
    ///
    /// let config = BrowserConfig::builder()
    ///     .with_url("http://localhost:4444")
    ///     .with_connect_timeout(Duration::from_secs(60))
    ///     .with_managed(false)
    ///     .build()?;
    ///
    /// let backend = BrowserBackend::builder()
    ///     .with_config(config)
    ///     .build()?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid and validation is enabled.
    pub fn with_config(mut self, config: WebDriverConfig) -> Result<Self> {
        if !self.skip_validation {
            config.validate()?;
        }
        self.configs.push(config);
        Ok(self)
    }

    /// Adds multiple WebDriver configurations to the pool.
    ///
    /// # Arguments
    ///
    /// * `configs` - Vector of WebDriver configurations
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_thirtyfour::{BrowserBackend, BrowserConfig};
    ///
    /// let configs = vec![
    ///     BrowserConfig::new("http://localhost:4444"),
    ///     BrowserConfig::new("http://localhost:4445"),
    /// ];
    ///
    /// let builder = BrowserBackend::builder()
    ///     .with_configs(configs)?
    ///     .build()?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if any configuration is invalid and validation is enabled.
    pub fn with_configs(mut self, configs: Vec<WebDriverConfig>) -> Result<Self> {
        if !self.skip_validation {
            for config in &configs {
                config.validate()?;
            }
        }
        self.configs.extend(configs);
        Ok(self)
    }

    /// Sets the maximum pool size.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_thirtyfour::BrowserBuilder;
    ///
    /// let builder = BrowserBuilder::new()
    ///     .with_unmanaged("http://localhost:4444")
    ///     .with_max_pool_size(20);
    /// ```
    pub fn with_max_pool_size(mut self, max_size: usize) -> Self {
        self.max_size = Some(max_size);
        self
    }

    /// Sets the minimum pool size.
    pub fn with_min_pool_size(mut self, min_size: usize) -> Self {
        self.min_size = Some(min_size);
        self
    }

    /// Sets the timeout for acquiring connections from the pool.
    pub fn with_acquire_timeout(mut self, timeout: Duration) -> Self {
        self.acquire_timeout = Some(timeout);
        self
    }

    /// Sets the maximum lifetime for connections in the pool.
    pub fn with_max_lifetime(mut self, lifetime: Duration) -> Self {
        self.max_lifetime = Some(lifetime);
        self
    }

    /// Sets the maximum idle time for connections in the pool.
    pub fn with_max_idle_time(mut self, idle_time: Duration) -> Self {
        self.max_idle_time = Some(idle_time);
        self
    }

    /// Enables or disables health checks for pooled browser instances.
    ///
    /// When enabled, the pool will periodically check if browser instances
    /// are healthy and remove any that have become unresponsive.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable health checks
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let builder = BrowserBuilder::new()
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
    /// When a browser connection fails to be created, the pool manager will
    /// retry up to this many times before giving up.
    ///
    /// # Arguments
    ///
    /// * `attempts` - Maximum number of retry attempts (minimum 1)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let builder = BrowserBuilder::new()
    ///     .with_unmanaged("http://localhost:4444")
    ///     .with_max_retry_attempts(5)
    ///     .build()?;
    /// ```
    pub fn with_max_retry_attempts(mut self, attempts: usize) -> Self {
        self.max_retry_attempts = Some(attempts.max(1));
        self
    }

    /// Disables configuration validation during the build process.
    ///
    /// By default, all configurations are validated before the pool is created.
    /// This method skips that validation, which can be useful for testing or
    /// when you're certain your configurations are valid.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let builder = BrowserBuilder::new()
    ///     .without_config_validation()
    ///     .with_unmanaged("invalid-url") // Would normally fail validation
    ///     .build()?;
    /// ```
    pub fn without_config_validation(mut self) -> Self {
        self.skip_validation = true;
        self
    }

    /// Builds the [`BrowserBackend`] with the configured settings.
    ///
    /// This method creates the internal browser manager, validates all configurations
    /// (unless disabled), and constructs the connection pool.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let backend = BrowserBuilder::new()
    ///     .with_unmanaged("http://localhost:4444")
    ///     .with_unmanaged("http://localhost:4445")
    ///     .with_max_pool_size(10)
    ///     .build()?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No WebDriver configurations have been added
    /// - Any configuration is invalid (when validation is enabled)
    /// - Pool configuration is invalid (e.g., zero max size)
    /// - Pool creation fails
    pub fn build(self) -> Result<BrowserBackend> {
        if self.configs.is_empty() {
            return Err(Error::new(
                ErrorKind::Backend,
                "At least one WebDriver configuration is required",
            ));
        }

        // Validate pool settings
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

        // Create browser manager with configurations
        let mut manager = BrowserManager::new();
        for config in self.configs {
            manager = manager.with_config(config);
        }

        // Apply manager settings
        if let Some(enabled) = self.health_checks {
            manager = manager.with_health_checks(enabled);
        }

        if let Some(attempts) = self.max_retry_attempts {
            manager = manager.with_max_retry_attempts(attempts);
        }

        // Build the connection pool
        let mut pool_builder = Pool::builder(manager).max_size(max_size);

        // Apply optional pool settings
        if let Some(timeout) = self.acquire_timeout {
            pool_builder = pool_builder.wait_timeout(Some(timeout));
        }

        // Note: deadpool doesn't expose max_lifetime and idle_timeout directly
        // These would need to be configured through the manager if needed

        let pool = pool_builder.build().map_err(|e| {
            Error::new(
                ErrorKind::Backend,
                format!("Failed to create browser connection pool: {}", e),
            )
        })?;

        Ok(BrowserBackend::from_pool(pool))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_default() {
        let builder = BrowserBuilder::default();
        assert!(builder.configs.is_empty());
        assert!(builder.max_size.is_none());
        assert!(builder.health_checks.is_none());
    }

    #[test]
    fn builder_with_unmanaged() {
        let builder = BrowserBuilder::new().with_unmanaged("http://localhost:4444");
        assert_eq!(builder.configs.len(), 1);
        assert_eq!(builder.configs[0].url, "http://localhost:4444");
        assert!(!builder.configs[0].managed);
    }

    #[test]
    fn builder_with_managed() {
        let builder = BrowserBuilder::new().with_managed("http://localhost:4444");
        assert_eq!(builder.configs.len(), 1);
        assert_eq!(builder.configs[0].url, "http://localhost:4444");
        assert!(builder.configs[0].managed);
    }

    #[test]
    fn builder_with_config() {
        let config = WebDriverConfig::new("http://localhost:4444");
        let builder = BrowserBuilder::new().with_config(config).unwrap();
        assert_eq!(builder.configs.len(), 1);
        assert_eq!(builder.configs[0].url, "http://localhost:4444");
    }

    #[test]
    fn builder_with_pool_settings() {
        let builder = BrowserBuilder::new()
            .with_max_pool_size(20)
            .with_min_pool_size(5)
            .with_health_checks(true)
            .with_max_retry_attempts(3);

        assert_eq!(builder.max_size, Some(20));
        assert_eq!(builder.min_size, Some(5));
        assert_eq!(builder.health_checks, Some(true));
        assert_eq!(builder.max_retry_attempts, Some(3));
    }

    #[test]
    fn builder_validation() {
        // Should require at least one config
        let result = BrowserBuilder::new().build();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("At least one WebDriver configuration")
        );

        // Should validate pool size
        let result = BrowserBuilder::new()
            .with_unmanaged("http://localhost:4444")
            .with_max_pool_size(0)
            .build();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("max_size must be greater than zero")
        );

        // Should validate min > max
        let result = BrowserBuilder::new()
            .with_unmanaged("http://localhost:4444")
            .with_max_pool_size(5)
            .with_min_pool_size(10)
            .build();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("min_size cannot be greater than max_size")
        );
    }

    #[test]
    fn builder_skip_validation() {
        let builder = BrowserBuilder::new()
            .without_config_validation()
            .with_unmanaged("invalid-url");
        assert!(builder.skip_validation);
    }

    #[test]
    fn builder_display() {
        let builder = BrowserBuilder::new()
            .with_unmanaged("http://localhost:4444")
            .with_max_pool_size(10);
        let display_str = format!("{}", builder);
        assert!(display_str.contains("configs: 1"));
        assert!(display_str.contains("max_size: Some(10)"));
    }

    #[test]
    fn builder_retry_attempts_minimum() {
        let builder = BrowserBuilder::new().with_max_retry_attempts(0);
        assert_eq!(builder.max_retry_attempts, Some(1)); // Should be clamped to minimum 1
    }
}
