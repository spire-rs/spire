use std::fmt;

use deadpool::managed::Pool;
use spire_core::Result;
use spire_core::backend::Backend;

use crate::error::BrowserError;
use crate::pool::{BrowserBuilder, BrowserConnection, WebDriverManager};

/// Browser backend implementation using Thirtyfour WebDriver.
///
/// `BrowserBackend` implements the [`Backend`] trait and creates [`BrowserConnection`]
/// instances that can perform browser automation using the Thirtyfour library.
///
/// This backend manages a pool of browser instances with health monitoring,
/// connection lifecycle management, and support for multiple WebDriver configurations.
///
/// # Examples
///
/// ```ignore
/// use spire_thirtyfour::BrowserBackend;
/// use spire_core::backend::Backend;
///
/// // Create with default configuration
/// let backend = BrowserBackend::default();
///
/// // Create with builder pattern
/// let backend = BrowserBackend::builder()
///     .with_unmanaged("http://localhost:4444")
///     .with_unmanaged("http://localhost:4445")
///     .build()
///     .unwrap();
///
/// // Get a connection
/// let connection = backend.connect().await.unwrap();
/// ```
#[derive(Clone)]
pub struct BrowserBackend {
    pool: Pool<WebDriverManager>,
}

impl BrowserBackend {
    /// Creates a new [`BrowserBackend`] from a browser pool.
    ///
    /// This allows you to use a pre-configured browser pool with specific
    /// WebDriver configurations, connection limits, and health monitoring settings.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_thirtyfour::BrowserBackend;
    /// use deadpool::managed::Pool;
    ///
    /// let pool = Pool::builder(manager).build().unwrap();
    /// let backend = BrowserBackend::from_pool(pool);
    /// ```
    pub fn from_pool(pool: Pool<WebDriverManager>) -> Self {
        Self { pool }
    }

    /// Creates a new [`BrowserBackend`] with default configuration.
    ///
    /// This creates a basic browser backend with a single localhost WebDriver connection.
    /// For production use, prefer using the builder pattern with specific configurations.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new [`BrowserBuilder`] for configuring the backend.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_thirtyfour::BrowserBackend;
    ///
    /// let backend = BrowserBackend::builder()
    ///     .with_unmanaged("http://localhost:4444")
    ///     .with_config(webdriver_config)
    ///     .with_pool_config(pool_config)
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn builder() -> BrowserBuilder {
        BrowserBuilder::default()
    }

    /// Returns pool statistics if available.
    ///
    /// This provides information about the current state of the browser pool,
    /// including active connections, idle connections, and pool capacity.
    pub fn pool_status(&self) -> deadpool::Status {
        self.pool.status()
    }
}

impl Default for BrowserBackend {
    /// Creates a default browser backend.
    ///
    /// Creates a default browser backend with a single localhost WebDriver connection
    /// at `http://127.0.0.1:4444`. This is suitable for development and testing.
    ///
    /// For production use, use [`BrowserBackend::builder`] with proper configuration.
    fn default() -> Self {
        BrowserBackend::builder()
            .with_unmanaged("http://127.0.0.1:4444")
            .build()
            .expect("Failed to create default BrowserBackend")
    }
}

impl From<Pool<WebDriverManager>> for BrowserBackend {
    fn from(pool: Pool<WebDriverManager>) -> Self {
        Self::from_pool(pool)
    }
}

impl fmt::Debug for BrowserBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = self.pool.status();
        f.debug_struct("BrowserBackend")
            .field("pool_size", &status.size)
            .field("available", &status.available)
            .finish_non_exhaustive()
    }
}

#[spire_core::async_trait]
impl Backend for BrowserBackend {
    type Client = BrowserConnection;

    /// Creates a new browser connection from this backend.
    ///
    /// This method acquires a browser instance from the pool and wraps it
    /// in a [`BrowserConnection`] that can perform individual browser automation
    /// tasks like navigation, content extraction, and JavaScript execution.
    ///
    /// The connection automatically handles browser lifecycle, health monitoring,
    /// and returns the browser instance to the pool when dropped.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No browser instances are available and the pool is exhausted
    /// - Unable to establish connection to WebDriver endpoints
    /// - Browser instance fails health checks
    async fn connect(&self) -> Result<Self::Client> {
        // pool.get() returns Object<WebDriverManager> which manages WebDriver instances
        let webdriver_client = self.pool.get().await.map_err(|_e| {
            let status = self.pool.status();
            spire_core::Error::from(BrowserError::pool_exhausted(status.size, status.available))
        })?;

        Ok(BrowserConnection::from_pooled(webdriver_client))
    }
}

#[cfg(test)]
mod tests {
    use spire_core::backend::Backend;

    use super::*;

    #[test]
    fn test_backend_creation() {
        let backend = BrowserBackend::default();
        assert!(backend.pool.status().max_size > 0);
    }

    #[test]
    fn test_builder_creation() {
        let backend = BrowserBackend::builder()
            .with_unmanaged("http://localhost:4444")
            .build()
            .unwrap();

        let status = backend.pool_status();
        assert!(status.max_size > 0);
    }

    #[test]
    fn test_debug_output() {
        let backend = BrowserBackend::default();
        let debug_str = format!("{:?}", backend);
        assert!(debug_str.contains("BrowserBackend"));
        assert!(debug_str.contains("pool_size"));
    }

    #[tokio::test]
    async fn test_connect() {
        let backend = BrowserBackend::builder()
            .with_unmanaged("http://localhost:4444")
            .build()
            .unwrap();

        // Note: This would fail without actual WebDriver server running
        // but verifies that the types and API work correctly
        let result = backend.connect().await;

        // In a real test environment with WebDriver running, this would succeed
        // For now, we just verify the error handling works
        match result {
            Ok(_connection) => {
                // Connection succeeded - WebDriver server is running
            }
            Err(e) => {
                // Expected when no WebDriver server is available
                assert!(e.to_string().contains("connection") || e.to_string().contains("pool"));
            }
        }
    }

    #[test]
    fn test_pool_status() {
        let backend = BrowserBackend::default();
        let status = backend.pool_status();

        // Default backend should have some configured capacity
        assert!(status.max_size > 0);
        assert!(status.available <= status.max_size);
    }
}
