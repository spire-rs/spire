#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]
//!
//! Thirtyfour-based browser automation backend for Spire.
//!
//! This crate provides [`BrowserBackend`], a WebDriver-based browser automation backend
//! that integrates with the Spire web scraping framework using the Thirtyfour library.
//!
//! # Features
//!
//! - **Connection Pooling**: Efficient management of browser instances with health monitoring
//! - **Multiple Browser Support**: Chrome, Firefox, Edge, Safari, and custom browsers
//! - **Configuration Management**: Comprehensive configuration for capabilities and timeouts
//! - **Error Handling**: Detailed error reporting with retry logic and debugging support
//! - **Performance Optimization**: Headless operation, image blocking, and resource management
//! - **Health Monitoring**: Connection lifecycle tracking and automatic cleanup
//!
//! # Architecture
//!
//! The crate is organized into several modules:
//!
//! - [`pool`] - Browser pool management and lifecycle
//! - [`client`] - Browser client and connection handling
//! - [`config`] - Configuration types and browser capabilities
//! - [`error`] - Enhanced error types and handling
//!
//! # Quick Start
//!
//! ```ignore
//! use spire_thirtyfour::{BrowserBackend, config::WebDriverConfig};
//! use spire_core::Client;
//!
//! # async fn example() -> spire_core::Result<()> {
//! // Create browser pool with multiple WebDriver endpoints
//! let backend = BrowserBackend::builder()
//!     .with_unmanaged("http://127.0.0.1:4444") // Chrome
//!     .with_unmanaged("http://127.0.0.1:4445") // Firefox
//!     .build()?;
//!
//! // Create Spire client
//! let worker = MyWorker::new();
//! let client = Client::new(backend, worker);
//!
//! // Run the scraping client
//! client.run().await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Advanced Configuration
//!
//! ```ignore
//! use spire_thirtyfour::{BrowserBackend, config::*};
//! use std::time::Duration;
//!
//! # async fn advanced_example() -> spire_core::Result<()> {
//! // Configure Chrome with custom capabilities using builder
//! let chrome_config = WebDriverConfig::builder()
//!     .with_url("http://localhost:4444")
//!     .with_browser(BrowserType::chrome())
//!     .with_connect_timeout(Duration::from_secs(30))
//!     .with_request_timeout(Duration::from_secs(60))
//!     .build()?;
//!
//! // Configure Firefox with performance optimizations
//! let firefox_config = WebDriverConfig::builder()
//!     .with_url("http://localhost:4445")
//!     .with_browser(BrowserType::firefox())
//!     .with_capabilities(BrowserType::firefox().performance_capabilities())
//!     .build()?;
//!
//! // Create backend with custom configurations
//! let backend = BrowserBackend::builder()
//!     .with_config(chrome_config)?
//!     .with_config(firefox_config)?
//!     .with_pool_config(
//!         PoolConfig::builder()
//!             .with_max_size(10)
//!             .with_min_size(2)
//!             .with_max_lifetime(Duration::from_secs(3600))
//!             .build()?
//!     )
//!     .build()?;
//!
//! // Use the configured backend...
//! # Ok(())
//! # }
//! ```
//!
//! # Browser Support
//!
//! The crate supports multiple browser types with optimized configurations:
//!
//! ## Chrome
//! ```ignore
//! let config = WebDriverConfig::new("http://localhost:9515")
//!     .with_browser(BrowserType::chrome());
//! ```
//!
//! ## Firefox
//! ```ignore
//! let config = WebDriverConfig::new("http://localhost:4444")
//!     .with_browser(BrowserType::firefox());
//! ```
//!
//! ## Edge
//! ```ignore
//! let config = WebDriverConfig::new("http://localhost:17556")
//!     .with_browser(BrowserType::edge());
//! ```
//!
//! ## Custom Browsers
//! ```ignore
//! let custom = BrowserType::custom("my-browser")
//!     .with_capability("customOption", serde_json::json!(true));
//!
//! let config = WebDriverConfig::new("http://localhost:8080")
//!     .with_browser(custom);
//! ```
//!
//! # Error Handling
//!
//! The crate provides comprehensive error handling with specific error types:
//!
//! ```ignore
//! use spire_thirtyfour::error::{BrowserError, NavigationErrorType};
//!
//! match result {
//!     Err(e) if e.is::<BrowserError>() => {
//!         let browser_err = e.downcast_ref::<BrowserError>().unwrap();
//!
//!         match browser_err {
//!             BrowserError::NavigationError { url, error_type, .. } => {
//!                 println!("Failed to navigate to {}: {:?}", url, error_type);
//!             }
//!             BrowserError::Timeout { operation, duration_secs } => {
//!                 println!("Operation '{}' timed out after {}s", operation, duration_secs);
//!             }
//!             _ => println!("Other browser error: {}", browser_err),
//!         }
//!     }
//!     Err(e) => println!("General error: {}", e),
//!     Ok(result) => { /* handle success */ }
//! }
//! ```
//!
//! # Health Monitoring
//!
//! Browser connections are continuously monitored for health:
//!
//! ```ignore
//! let backend = BrowserBackend::builder()
//!     .with_health_checks(true)
//!     .with_pool_config(
//!         PoolConfig::builder()
//!             .with_health_check_interval(Duration::from_secs(60))
//!             .with_max_idle_time(Duration::from_secs(300))
//!             .build()?
//!     )
//!     .build()?;
//! ```
//!
//! # Performance Tuning
//!
//! Several options are available for performance optimization:
//!
//! ## Headless Operation
//! ```ignore
//! let caps = BrowserType::chrome().headless_capabilities();
//! let config = WebDriverConfig::new("http://localhost:4444")
//!     .with_capabilities(caps);
//! ```
//!
//! ## Resource Blocking
//! ```ignore
//! let caps = BrowserType::chrome().performance_capabilities();
//! let config = WebDriverConfig::new("http://localhost:4444")
//!     .with_capabilities(caps); // Blocks images, plugins, etc.
//! ```
//!
//! ## Connection Limits
//! ```ignore
//! let pool_config = PoolConfig::builder()
//!     .with_max_size(20)           // Maximum browsers
//!     .with_acquire_timeout(Duration::from_secs(30)) // Wait time for browser
//!     .with_max_lifetime(Duration::from_secs(1800))  // Recycle after 30min
//!     .build()?;
//! ```

#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

// Core modules
pub mod client;
pub mod config;
pub mod error;
/// Browser pool management and connection lifecycle.
///
/// This module provides the core [`BrowserBackend`] and [`BrowserPool`] types and supporting infrastructure
/// for managing collections of WebDriver browser instances. It includes:
///
/// - [`BrowserBackend`] - Main backend interface implementing the Spire Backend trait
/// - [`BrowserPool`] - Internal pool for managing browser connections
/// - `builder` - Builder pattern for configuring browser pools
/// - `manager` - Internal pool management and connection lifecycle
pub mod pool;

// Re-export thirtyfour types for convenience
pub use thirtyfour::{WebDriver, WebElement};

pub use crate::client::{BrowserBackend, ClientConfig};
pub use crate::config::capabilities::{self, CapabilitiesBuilder};
pub use crate::config::{
    BrowserType, PoolConfig, PoolConfigBuilder, WebDriverConfig, WebDriverConfigBuilder,
};
pub use crate::error::{BrowserError, NavigationErrorType};
pub use crate::pool::{BrowserBuilder, BrowserConnection, BrowserPool};

/// Prelude module for convenient imports.
///
/// This module re-exports the most commonly used types and traits.
///
/// # Examples
///
/// ```ignore
/// use spire_thirtyfour::prelude::*;
///
/// let backend = BrowserBackend::builder()
///     .with_unmanaged("http://localhost:4444")
///     .build()?;
/// ```
#[doc(hidden)]
pub mod prelude;

#[cfg(test)]
mod tests {
    // Imports for tests would go here when the integration test is re-enabled

    use super::*;

    #[test]
    fn build_browser_backend() {
        let _backend = BrowserBackend::builder()
            .with_unmanaged("http://127.0.0.1:4444")
            .build();
    }

    #[test]
    fn browser_types() {
        assert_eq!(BrowserType::Chrome.browser_name(), "chrome");
        assert_eq!(BrowserType::Firefox.browser_name(), "firefox");
        assert_eq!(BrowserType::Edge.browser_name(), "MicrosoftEdge");
        assert_eq!(BrowserType::Safari.browser_name(), "safari");
    }

    #[test]
    fn webdriver_config_validation() {
        let config = WebDriverConfig::new("http://localhost:4444");
        assert!(config.validate().is_ok());

        let invalid_config = WebDriverConfig::new("");
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn pool_config_validation() {
        let config = PoolConfig::builder()
            .with_max_size(10_usize)
            .with_min_size(2_usize)
            .build()
            .expect("Should build successfully");
        assert!(config.validate().is_ok());

        let invalid_result = PoolConfig::builder().with_max_size(0_usize).build();
        assert!(invalid_result.is_err());
    }

    // FIXME: Commented out due to Service trait constraint issues with BrowserConnection
    // #[tokio::test]
    // async fn integration_test_noop() -> Result<()> {
    //     // This test verifies the integration without requiring actual WebDriver
    //     let backend = BrowserBackend::builder()
    //         .with_unmanaged("http://127.0.0.1:4444")
    //         .with_unmanaged("http://127.0.0.1:4445")
    //         .build()
    //         .expect("Failed to build backend");

    //     // Create a no-op worker for testing
    //     let worker = Noop::default();

    //     let request = Request::get("https://example.com/")
    //         .body(())
    //         .expect("Failed to create request");

    //     let client = Client::new(backend, worker)
    //         .with_request_queue(InMemDataset::stack())
    //         .with_dataset(InMemDataset::<u64>::new())
    //         .with_initial_request(request);

    //     // Note: This would fail without actual WebDriver servers running
    //     // but verifies that the API compiles and types work correctly
    //     let _dataset = client.dataset::<u64>();

    //     // Skip actual execution in tests
    //     // let _ = client.run().await?;

    //     Ok(())
    // }

    #[test]
    fn capabilities_builder() {
        let caps = CapabilitiesBuilder::new()
            .browser_name("chrome")
            .browser_version("latest")
            .accept_insecure_certs(true)
            .page_load_strategy(capabilities::page_load_strategy::NORMAL)
            .build();

        assert_eq!(caps["browserName"], serde_json::json!("chrome"));
        assert_eq!(caps["browserVersion"], serde_json::json!("latest"));
        assert_eq!(caps["acceptInsecureCerts"], serde_json::json!(true));
        assert_eq!(caps["pageLoadStrategy"], serde_json::json!("normal"));
    }

    #[test]
    fn browser_error_categories() {
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
    fn error_retryability() {
        assert!(
            BrowserError::connection_failed("http://localhost:4444", "Connection refused")
                .is_retryable()
        );

        assert!(!BrowserError::configuration("Invalid config", None).is_retryable());

        assert!(BrowserError::timeout("navigation", 30).is_retryable());
    }
}
