use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use deadpool::managed::{Manager, Metrics, RecycleResult};
use derive_builder::Builder;
use derive_more::Display;
use spire_core::{Error, ErrorKind, Result};
use thirtyfour::{DesiredCapabilities, WebDriver};

use crate::client::connection::BrowserConnection;
use crate::config::{PoolConfig, WebDriverConfig};
use crate::error::BrowserError;

/// Pool manager for browser connections.
///
/// This type implements the `deadpool::managed::Manager` trait to handle
/// creation, health checking, and recycling of browser instances in the pool.
///
/// The manager supports both managed (spawned browser processes) and unmanaged
/// (external WebDriver servers) connection types, with configurable capabilities
/// and health monitoring.
///
/// # Examples
///
/// ```ignore
/// use spire_thirtyfour::pool::manager::BrowserManager;
/// use spire_thirtyfour::config::WebDriverConfig;
///
/// let config = WebDriverConfig::new("http://localhost:4444");
/// let manager = BrowserManager::new()
///     .with_config(config)
///     .with_pool_config(PoolConfig::default());
/// ```
#[derive(Clone)]
pub struct BrowserManager {
    /// WebDriver server configurations indexed by connection ID
    configs: Arc<HashMap<u64, WebDriverConfig>>,
    /// Pool configuration settings
    pool_config: PoolConfig,
    /// Connection ID counter
    connection_counter: Arc<AtomicU64>,
    /// Health check configuration
    health_check_enabled: bool,
    /// Maximum number of retry attempts for connection creation
    max_retry_attempts: usize,
}

impl BrowserManager {
    /// Creates a new [`BrowserManager`] with default settings.
    pub fn new() -> Self {
        Self {
            configs: Arc::new(HashMap::new()),
            pool_config: PoolConfig::default(),
            connection_counter: Arc::new(AtomicU64::new(1)),
            health_check_enabled: true,
            max_retry_attempts: 3,
        }
    }

    /// Adds a WebDriver configuration to the manager.
    ///
    /// Each configuration represents a potential browser connection endpoint.
    /// The manager will use these configurations to create browser instances
    /// as needed by the pool.
    ///
    /// # Arguments
    ///
    /// * `config` - WebDriver configuration including URL, capabilities, and timeouts
    ///
    /// # Returns
    ///
    /// Returns the updated manager for method chaining.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let manager = BrowserManager::new()
    ///     .with_config(WebDriverConfig::new("http://localhost:4444"))
    ///     .with_config(WebDriverConfig::new("http://localhost:4445"));
    /// ```
    pub fn with_config(mut self, config: WebDriverConfig) -> Result<Self> {
        // Validate the configuration first
        config.validate()?;

        let connection_id = self.connection_counter.fetch_add(1, Ordering::SeqCst);

        // Clone the configs HashMap, add the new config, and replace
        let mut configs = (*self.configs).clone();
        configs.insert(connection_id, config);
        self.configs = Arc::new(configs);

        Ok(self)
    }

    /// Sets the pool configuration.
    pub fn with_pool_config(mut self, pool_config: PoolConfig) -> Self {
        self.pool_config = pool_config;
        self
    }

    /// Enables or disables health checks for browser connections.
    pub fn with_health_checks(mut self, enabled: bool) -> Self {
        self.health_check_enabled = enabled;
        self
    }

    /// Sets the maximum number of retry attempts for connection creation.
    pub fn with_max_retry_attempts(mut self, attempts: usize) -> Self {
        self.max_retry_attempts = attempts;
        self
    }

    /// Creates a WebDriver instance using the provided configuration.
    async fn create_webdriver(&self, config: &WebDriverConfig) -> Result<WebDriver> {
        let capabilities = DesiredCapabilities::chrome();

        // Create WebDriver with retry logic
        let mut last_error: Option<BrowserError> = None;

        for attempt in 1..=self.max_retry_attempts {
            match WebDriver::new(config.base_url(), capabilities.clone()).await {
                Ok(driver) => {
                    // Verify the session is working
                    match self.verify_session(&driver).await {
                        Ok(()) => return Ok(driver),
                        Err(e) => {
                            last_error = Some(e);
                            if attempt < self.max_retry_attempts {
                                // Brief delay before retry
                                tokio::time::sleep(Duration::from_millis(100 * attempt as u64))
                                    .await;
                                continue;
                            }
                        }
                    }
                }
                Err(e) => {
                    last_error = Some(BrowserError::connection_failed(&config.url, e));
                    if attempt < self.max_retry_attempts {
                        // Exponential backoff
                        let delay = Duration::from_millis(500 * (2_u64.pow(attempt as u32 - 1)));
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| {
                BrowserError::connection_failed(
                    &config.url,
                    "Unknown error after all retry attempts",
                )
            })
            .into())
    }

    /// Verifies that a WebDriver session is functioning correctly.
    async fn verify_session(&self, driver: &WebDriver) -> Result<(), BrowserError> {
        // Try to get the current URL - this is a basic health check
        match tokio::time::timeout(Duration::from_secs(5), driver.current_url()).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(BrowserError::webdriver(
                e,
                Some("session verification".to_string()),
            )),
            Err(_) => Err(BrowserError::timeout("session verification", 5)),
        }
    }

    /// Performs a comprehensive health check on a browser connection.
    async fn health_check(&self, connection: &BrowserConnection) -> bool {
        if !self.health_check_enabled {
            return true;
        }

        // Check if the connection is too old
        if let Some(max_lifetime) = self.pool_config.max_lifetime
            && connection.created_at.elapsed() > max_lifetime
        {
            return false;
        }

        // Check if the connection has been idle too long
        if let Some(max_idle) = self.pool_config.max_idle_time
            && connection.last_used.elapsed() > max_idle
        {
            return false;
        }

        // Verify the WebDriver session is still active
        match self.verify_session(&connection.client).await {
            Ok(()) => true,
            Err(_) => false,
        }
    }

    /// Selects the best configuration for creating a new connection.
    ///
    /// This implementation uses a simple round-robin approach, but could be
    /// enhanced with load balancing algorithms in the future.
    fn select_config(&self) -> Option<&WebDriverConfig> {
        if self.configs.is_empty() {
            return None;
        }

        // For now, just pick the first available config
        // TODO: Implement proper load balancing/selection strategy
        self.configs.values().next()
    }

    /// Cleans up a browser connection and closes the session.
    #[allow(dead_code)]
    async fn cleanup_connection(&self, connection: &mut BrowserConnection) {
        // Attempt to quit the WebDriver session gracefully
        if let Err(e) = connection.client.clone().quit().await {
            // Log the error but don't fail - the connection might already be dead
            eprintln!(
                "Warning: Failed to quit WebDriver session {}: {}",
                connection.id, e
            );
        }
    }
}

impl Default for BrowserManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Manager for BrowserManager {
    type Error = Error;
    type Type = BrowserConnection;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        // Select a configuration to use for this connection
        let config = self.select_config().ok_or_else(|| {
            Error::new(
                ErrorKind::Backend,
                "No WebDriver configurations available for creating browser connection",
            )
        })?;

        // Create the WebDriver instance
        let driver = self.create_webdriver(config).await?;

        // Generate a unique connection ID
        let connection_id = self.connection_counter.fetch_add(1, Ordering::SeqCst);

        // Create the connection wrapper
        let connection = BrowserConnection::new(connection_id, driver, config.clone());

        Ok(connection)
    }

    async fn recycle(
        &self,
        obj: &mut Self::Type,
        _metrics: &Metrics,
    ) -> RecycleResult<Self::Error> {
        // Update the last used timestamp
        obj.mark_used();

        // Perform health check
        if !self.health_check(obj).await {
            return Ok(());
        }

        // Check pool metrics for additional recycling decisions
        if let Some(max_lifetime) = self.pool_config.max_lifetime
            && obj.created_at.elapsed() > max_lifetime
        {
            return Ok(());
        }

        // Connection appears healthy and within limits
        RecycleResult::Ok(())
    }

    fn detach(&self, _obj: &mut Self::Type) {
        // Clean up the connection when it's being removed from the pool
        // Note: We can't use async here or mark as unhealthy since we only have &mut
        // The actual cleanup will happen when the connection is dropped
    }
}

/// Builder for creating a configured BrowserManager.
impl From<spire_core::Error> for BrowserManagerConfigBuilderError {
    fn from(err: spire_core::Error) -> Self {
        Self::ValidationError(err.to_string())
    }
}

impl From<BrowserManagerConfigBuilderError> for spire_core::Error {
    fn from(err: BrowserManagerConfigBuilderError) -> Self {
        Error::new(ErrorKind::Backend, err.to_string())
    }
}

#[derive(Debug, Clone, Builder)]
#[builder(
    name = "BrowserManagerConfigBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(validate = "Self::validate_config")
)]
/// Configuration for browser connection manager.
///
/// This struct contains all the settings needed to configure a browser pool manager,
/// including WebDriver configurations, pool settings, health check options, and retry behavior.
pub struct BrowserManagerConfig {
    /// WebDriver configurations to add to the manager
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
}

impl BrowserManagerConfigBuilder {
    fn validate_config(&self) -> Result<()> {
        let binding = Vec::new();
        let configs = self.configs.as_ref().unwrap_or(&binding);

        // Validate all configurations
        for config in configs {
            config.validate()?;
        }

        // Validate pool configuration if provided
        if let Some(Some(pool_config)) = self.pool_config.as_ref() {
            pool_config.validate()?;
        }

        // Validate retry attempts
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

impl BrowserManagerConfig {
    /// Creates a new builder for browser manager configuration.
    pub fn builder() -> BrowserManagerConfigBuilder {
        BrowserManagerConfigBuilder::default()
    }

    /// Builds a BrowserManager from this configuration.
    pub fn build_manager(self) -> Result<BrowserManager> {
        let mut manager = BrowserManager::new();

        // Add all configurations
        for config in self.configs {
            manager = manager.with_config(config)?;
        }

        // Apply optional settings
        if let Some(pool_config) = self.pool_config {
            manager = manager.with_pool_config(pool_config);
        }

        if let Some(health_checks) = self.health_checks {
            manager = manager.with_health_checks(health_checks);
        }

        if let Some(max_retries) = self.max_retry_attempts {
            manager = manager.with_max_retry_attempts(max_retries);
        }

        Ok(manager)
    }
}

/// Legacy builder for backward compatibility.
#[derive(Debug, Default, Display)]
#[display(fmt = "BrowserManagerBuilder(configs: {})", "configs.len()")]
pub struct BrowserManagerBuilder {
    configs: Vec<WebDriverConfig>,
    pool_config: Option<PoolConfig>,
    health_checks: Option<bool>,
    max_retry_attempts: Option<usize>,
}

impl BrowserManagerBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a WebDriver configuration.
    pub fn add_config(mut self, config: WebDriverConfig) -> Self {
        self.configs.push(config);
        self
    }

    /// Adds multiple WebDriver configurations.
    pub fn add_configs(mut self, configs: Vec<WebDriverConfig>) -> Self {
        self.configs.extend(configs);
        self
    }

    /// Sets the pool configuration.
    pub fn pool_config(mut self, config: PoolConfig) -> Self {
        self.pool_config = Some(config);
        self
    }

    /// Enables or disables health checks.
    pub fn health_checks(mut self, enabled: bool) -> Self {
        self.health_checks = Some(enabled);
        self
    }

    /// Sets the maximum retry attempts.
    pub fn max_retry_attempts(mut self, attempts: usize) -> Self {
        self.max_retry_attempts = Some(attempts);
        self
    }

    /// Builds the BrowserManager using the new builder pattern internally.
    pub fn build(self) -> Result<BrowserManager> {
        BrowserManagerConfig::builder()
            .with_configs(self.configs)
            .with_pool_config(self.pool_config.unwrap_or_default())
            .with_health_checks(self.health_checks.unwrap_or(true))
            .with_max_retry_attempts(self.max_retry_attempts.unwrap_or(3))
            .build()
            .map_err(Error::from)?
            .build_manager()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::browser::BrowserType;

    #[test]
    fn manager_creation() {
        let manager = BrowserManager::new();
        assert!(manager.configs.is_empty());
        assert_eq!(manager.max_retry_attempts, 3);
        assert!(manager.health_check_enabled);
    }

    #[test]
    fn manager_with_config() {
        let config =
            WebDriverConfig::new("http://localhost:4444").with_browser(BrowserType::Chrome);

        let manager = BrowserManager::new()
            .with_config(config)
            .expect("Config should be valid");

        assert_eq!(manager.configs.len(), 1);
    }

    #[test]
    fn manager_builder() {
        let config1 = WebDriverConfig::new("http://localhost:4444");
        let config2 = WebDriverConfig::new("http://localhost:4445");

        let manager = BrowserManagerBuilder::new()
            .add_config(config1)
            .add_config(config2)
            .health_checks(false)
            .max_retry_attempts(5)
            .build()
            .expect("Should build successfully");

        assert_eq!(manager.configs.len(), 2);
        assert!(!manager.health_check_enabled);
        assert_eq!(manager.max_retry_attempts, 5);
    }

    #[test]
    fn manager_config_builder() {
        let config1 = WebDriverConfig::new("http://localhost:4444");
        let config2 = WebDriverConfig::new("http://localhost:4445");

        let config = BrowserManagerConfig::builder()
            .with_configs(vec![config1, config2])
            .with_health_checks(false)
            .with_max_retry_attempts(5_usize)
            .build()
            .expect("Should build successfully");

        assert_eq!(config.configs.len(), 2);
        assert_eq!(config.health_checks, Some(false));
        assert_eq!(config.max_retry_attempts, Some(5));
    }

    #[test]
    fn manager_config_validation() {
        // Empty configs should pass (default behavior)
        let config = BrowserManagerConfig::builder()
            .build()
            .expect("Should build successfully");
        assert!(config.configs.is_empty());

        // Zero max retries should fail validation
        let result = BrowserManagerConfig::builder()
            .with_max_retry_attempts(0_usize)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn invalid_config_rejected() {
        let invalid_config = WebDriverConfig {
            url: String::new(), // Invalid: empty URL
            ..Default::default()
        };

        let result = BrowserManager::new().with_config(invalid_config);
        assert!(result.is_err());
    }
}
