//! # Spire `ThirtyFour` - `WebDriver` Backend for Spire
//!
//! This crate provides a WebDriver-based backend for the Spire web scraping framework,
//! built on top of the `thirtyfour` crate. It offers browser automation capabilities
//! with connection pooling, health monitoring, and robust error handling.
//!
//! ## Quick Start
//!
//! ```no_run
//! use spire_thirtyfour::BrowserBackend;
//! use spire_core::backend::Backend;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a backend with default configuration
//!     let backend = BrowserBackend::builder()
//!         .with_unmanaged("http://localhost:4444")
//!         .build()?;
//!
//!     // Get a browser connection
//!     let connection = backend.connect().await?;
//!
//!     // Use the connection (implements spire_core::backend::Client)
//!     // connection.navigate("https://example.com").await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Core Components
//!
//! ### Backend and Connections
//! - [`BrowserBackend`] - Main backend implementation with connection pooling
//! - [`BrowserConnection`] - Individual browser connection wrapper
//! - [`BrowserBuilder`] - Builder for configuring the backend
//!
//! ### Configuration
//! - [`BrowserConfig`] - `WebDriver` connection configuration
//! - [`BrowserBehaviorConfig`] - Browser behavior settings
//!
//! ### Error Handling
//! - [`BrowserError`] - Comprehensive error types for browser operations
//! - [`NavigationErrorType`] - Specific navigation error classifications
//! - [`BrowserResult`] - Type alias for `Result<T, BrowserError>`
//!
//! ## Advanced Configuration
//!
//! ```no_run
//! use spire_thirtyfour::{BrowserBackend, BrowserConfig};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Configure individual browser settings
//!     let chrome_config = BrowserConfig::builder()
//!         .with_url("http://localhost:4444")
//!         .with_connect_timeout(Duration::from_secs(30))
//!         .build()?;
//!
//!     // Create backend with custom pool settings
//!     let backend = BrowserBackend::builder()
//!         .with_config(chrome_config)?
//!         .with_max_pool_size(10)
//!         .with_health_checks(true)
//!         .with_max_retry_attempts(3)
//!         .build()?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Multiple `WebDriver` Endpoints
//!
//! ```no_run
//! use spire_thirtyfour::BrowserBackend;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let backend = BrowserBackend::builder()
//!         .with_unmanaged("http://localhost:4444")  // Chrome
//!         .with_unmanaged("http://localhost:4445")  // Firefox
//!         .with_managed("http://localhost:4446")    // Managed instance
//!         .with_max_pool_size(15)
//!         .build()?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Feature Flags
//!
//! This crate requires exactly one TLS implementation:
//! - `rustls-tls` - Use rustls for TLS (recommended)
//! - `native-tls` - Use system's native TLS implementation
//!
//! ## Integration with Spire
//!
//! This backend integrates seamlessly with the Spire framework:
//!
//! ```no_run
//! use spire_thirtyfour::BrowserBackend;
//! use spire_core::backend::Backend;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let backend = BrowserBackend::default();
//!     let connection = backend.connect().await?;
//!
//!     // Use the connection for browser automation
//!     // connection.navigate("https://example.com").await?;
//!
//!     Ok(())
//! }
//! ```

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

// Ensure at least one TLS feature is enabled
#[cfg(not(any(feature = "rustls-tls", feature = "native-tls")))]
compile_error!("At least one TLS feature must be enabled: 'rustls-tls' or 'native-tls'");

mod client;
mod error;
mod pool;

// Re-export entire thirtyfour crate for convenience
pub use thirtyfour;

// Main public API exports
pub use crate::client::{BrowserBackend, BrowserConfig, BrowserConfigBuilder};
pub use crate::error::{BrowserError, BrowserResult, NavigationErrorType};
pub use crate::pool::{BrowserBehaviorConfig, BrowserBuilder, BrowserConnection};

pub mod prelude;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_browser_backend() {
        let _backend = BrowserBackend::builder()
            .with_unmanaged("http://127.0.0.1:4444")
            .build();
    }

    #[test]
    fn browser_config_validation() {
        let config = BrowserConfig::new("http://localhost:4444");
        assert!(config.validate().is_ok());

        let invalid_config = BrowserConfig::new("");
        assert!(invalid_config.validate().is_err());
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
