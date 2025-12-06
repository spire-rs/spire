#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

// Ensure at least one TLS feature is enabled
#[cfg(not(any(feature = "rustls-tls", feature = "native-tls")))]
compile_error!("At least one TLS feature must be enabled: 'rustls-tls' or 'native-tls'");

pub mod client;
pub mod error;
pub mod pool;

// Re-export thirtyfour types for convenience
// Re-export thirtyfour crate
pub use thirtyfour;
pub use thirtyfour::{WebDriver, WebElement};

pub use crate::client::{
    BrowserBackend, BrowserConfig, BrowserConfigBuilder, PoolConfig, PoolConfigBuilder,
};
pub use crate::error::{BrowserError, NavigationErrorType};
pub use crate::pool::{BrowserBehaviorConfig, BrowserBuilder, BrowserConnection, BrowserPool};

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
    fn browser_config_validation() {
        let config = BrowserConfig::new("http://localhost:4444");
        assert!(config.validate().is_ok());

        let invalid_config = BrowserConfig::new("");
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
