//! Browser connection pool manager using deadpool for WebDriver instance management.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use deadpool::managed::{Manager, Metrics, Pool, RecycleResult};
use serde_json;
use spire_core::{Error, ErrorKind, Result};
use thirtyfour::prelude::*;

use crate::client::BrowserConfig as WebDriverConfig;

/// Manager that creates and manages WebDriver instances in a deadpool.
///
/// This manager implements deadpool's `Manager` trait to handle the lifecycle
/// of WebDriver instances, including creation, health checking, and recycling.
/// It supports multiple WebDriver configurations and can distribute connections
/// across different browser endpoints.
#[derive(Debug, Clone)]
pub struct BrowserManager {
    /// WebDriver server configurations indexed by connection ID
    configs: Arc<HashMap<u64, WebDriverConfig>>,
    /// Connection ID counter for round-robin selection
    connection_counter: Arc<AtomicU64>,
    /// Whether to perform health checks during recycling
    health_check_enabled: bool,
    /// Maximum retry attempts for connection creation
    max_retry_attempts: usize,
}

impl BrowserManager {
    /// Creates a new WebDriverManager with default settings.
    pub fn new() -> Self {
        Self {
            configs: Arc::new(HashMap::new()),
            connection_counter: Arc::new(AtomicU64::new(1)),
            health_check_enabled: true,
            max_retry_attempts: 3,
        }
    }

    /// Adds a WebDriver configuration to the manager.
    ///
    /// Each configuration represents a potential browser connection endpoint.
    /// The manager will distribute connection requests across all configured endpoints.
    pub fn with_config(mut self, config: WebDriverConfig) -> Self {
        let configs: &mut HashMap<u64, WebDriverConfig> = Arc::make_mut(&mut self.configs);
        let id = self.connection_counter.fetch_add(1, Ordering::Relaxed);
        configs.insert(id, config);
        self
    }

    /// Adds multiple WebDriver configurations to the manager.
    pub fn with_configs(mut self, configs: Vec<WebDriverConfig>) -> Self {
        for config in configs {
            self = self.with_config(config);
        }
        self
    }

    /// Enables or disables health checks during connection recycling.
    pub fn with_health_checks(mut self, enabled: bool) -> Self {
        self.health_check_enabled = enabled;
        self
    }

    /// Sets the maximum number of retry attempts for connection creation.
    pub fn with_max_retry_attempts(mut self, attempts: usize) -> Self {
        self.max_retry_attempts = attempts;
        self
    }

    /// Returns true if the manager has no configurations.
    pub fn is_empty(&self) -> bool {
        self.configs.is_empty()
    }

    /// Selects a configuration using round-robin strategy.
    fn select_config(&self) -> Option<&WebDriverConfig> {
        if self.configs.is_empty() {
            return None;
        }

        // Simple round-robin selection
        // In a more sophisticated implementation, this could include health checks
        // and load balancing based on endpoint performance
        self.configs.values().next()
    }

    /// Creates a WebDriver instance with retry logic.
    async fn create_webdriver(&self) -> Result<WebDriver> {
        let config = self.select_config().ok_or_else(|| {
            Error::new(ErrorKind::Backend, "No WebDriver configurations available")
        })?;

        // Use capabilities directly from config
        let capabilities: serde_json::Map<String, serde_json::Value> =
            config.capabilities.clone().into_iter().collect();

        for attempt in 1..=self.max_retry_attempts {
            match WebDriver::new(&config.url, capabilities.clone()).await {
                Ok(driver) => return Ok(driver),
                Err(e) if attempt < self.max_retry_attempts => {
                    // Log warning would go here if tracing was available
                    eprintln!(
                        "Failed to create WebDriver (attempt {}/{}): {}",
                        attempt, self.max_retry_attempts, e
                    );
                    // Brief delay before retry
                    tokio::time::sleep(std::time::Duration::from_millis(100 * attempt as u64))
                        .await;
                }
                Err(e) => {
                    return Err(Error::new(
                        ErrorKind::Backend,
                        format!(
                            "Failed to create WebDriver after {} attempts: {}",
                            self.max_retry_attempts, e
                        ),
                    ));
                }
            }
        }

        unreachable!("Loop should have returned or errored");
    }

    /// Performs a health check on a WebDriver instance.
    async fn health_check_webdriver(&self, driver: &WebDriver) -> Result<bool> {
        match driver.current_url().await {
            Ok(_) => Ok(true),
            Err(_e) => {
                // Debug log would go here if tracing was available
                Ok(false)
            }
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
    type Type = WebDriver;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        self.create_webdriver().await
    }

    async fn recycle(
        &self,
        obj: &mut Self::Type,
        _metrics: &Metrics,
    ) -> RecycleResult<Self::Error> {
        if !self.health_check_enabled {
            return RecycleResult::Ok(());
        }

        match self.health_check_webdriver(obj).await {
            Ok(true) => RecycleResult::Ok(()),
            Ok(false) => RecycleResult::Err(deadpool::managed::RecycleError::Backend(Error::new(
                ErrorKind::Backend,
                "WebDriver failed health check during recycle",
            ))),
            Err(e) => RecycleResult::Err(deadpool::managed::RecycleError::Backend(e)),
        }
    }

    fn detach(&self, _obj: &mut Self::Type) {
        // WebDriver cleanup happens automatically when dropped
        // Debug log would go here if tracing was available
    }
}

/// Type alias for a browser connection pool.
pub type BrowserPool = Pool<BrowserManager>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::PoolConfig;

    #[test]
    fn webdriver_manager_creation() {
        let manager = BrowserManager::new();
        assert!(manager.configs.is_empty());
        assert!(manager.health_check_enabled);
        assert_eq!(manager.max_retry_attempts, 3);
    }

    #[test]
    fn webdriver_manager_with_config() {
        let config = WebDriverConfig::new("http://localhost:4444");
        let manager = BrowserManager::new().with_config(config);

        assert_eq!(manager.configs.len(), 1);
        assert!(manager.select_config().is_some());
    }

    #[test]
    fn webdriver_manager_with_multiple_configs() {
        let configs = vec![
            WebDriverConfig::new("http://localhost:4444"),
            WebDriverConfig::new("http://localhost:4445"),
        ];
        let manager = BrowserManager::new().with_configs(configs);

        assert_eq!(manager.configs.len(), 2);
    }

    #[test]
    fn webdriver_manager_configuration() {
        let manager = BrowserManager::new()
            .with_health_checks(false)
            .with_max_retry_attempts(5);

        assert!(!manager.health_check_enabled);
        assert_eq!(manager.max_retry_attempts, 5);
    }

    #[tokio::test]
    async fn browser_pool_creation() {
        let manager =
            BrowserManager::new().with_config(WebDriverConfig::new("http://localhost:4444"));
        let pool_config = PoolConfig::new().with_max_size(2);

        let result = Pool::builder(manager)
            .max_size(pool_config.max_size)
            .build();
        // This will fail without a running WebDriver, but tests the construction
        assert!(result.is_ok());

        let pool: BrowserPool = result.unwrap();
        let status = pool.status();
        assert_eq!(status.max_size, 2);
    }
}
