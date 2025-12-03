//! Browser client implementation with backend/connection architecture.

use std::time::Duration;

use derive_builder::Builder;
use spire_core::Error;

mod backend;

pub use backend::BrowserBackend;

/// Configuration for browser client behavior.
#[derive(Debug, Clone, Builder)]
#[builder(
    name = "ClientConfigBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(validate = "Self::validate_config", error = "ClientConfigError")
)]
pub struct ClientConfig {
    /// Timeout for page navigation
    #[builder(default = "Duration::from_secs(30)")]
    pub navigation_timeout: Duration,
    /// Timeout for element finding operations
    #[builder(default = "Duration::from_secs(10)")]
    pub element_timeout: Duration,
    /// Whether to extract full HTML content
    #[builder(default = "true")]
    pub extract_html: bool,
    /// Whether to extract text content
    #[builder(default = "false")]
    pub extract_text: bool,
    /// Whether to execute JavaScript for dynamic content
    #[builder(default = "true")]
    pub enable_javascript: bool,
    /// Whether to wait for page load completion
    #[builder(default = "true")]
    pub wait_for_load: bool,
    /// Custom user agent string
    #[builder(default = "None")]
    pub user_agent: Option<String>,
    /// Maximum response body size in bytes
    #[builder(default = "10 * 1024 * 1024")]
    pub max_response_size: usize,
    /// Whether to capture screenshots on errors
    #[builder(default = "false")]
    pub capture_screenshots: bool,
}

/// Error type for client configuration validation.
#[derive(Debug)]
pub struct ClientConfigError(String);

impl std::fmt::Display for ClientConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Client configuration error: {}", self.0)
    }
}

impl std::error::Error for ClientConfigError {}

impl From<Error> for ClientConfigError {
    fn from(e: Error) -> Self {
        Self(e.to_string())
    }
}

impl From<String> for ClientConfigError {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ClientConfigError {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl ClientConfigBuilder {
    fn validate_config(&self) -> Result<(), ClientConfigError> {
        if let Some(timeout) = &self.navigation_timeout
            && timeout.as_secs() == 0
        {
            return Err("Navigation timeout must be greater than 0".into());
        }

        if let Some(timeout) = &self.element_timeout
            && timeout.as_secs() == 0
        {
            return Err("Element timeout must be greater than 0".into());
        }

        if let Some(size) = &self.max_response_size
            && *size == 0
        {
            return Err("Max response size must be greater than 0".into());
        }

        if let Some(user_agent) = &self.user_agent
            && let Some(ua) = user_agent
            && ua.trim().is_empty()
        {
            return Err("User agent cannot be empty".into());
        }

        Ok(())
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            navigation_timeout: Duration::from_secs(30),
            element_timeout: Duration::from_secs(10),
            extract_html: true,
            extract_text: false,
            enable_javascript: true,
            wait_for_load: true,
            user_agent: None,
            max_response_size: 10 * 1024 * 1024, // 10 MB
            capture_screenshots: false,
        }
    }
}

impl ClientConfig {
    /// Creates a new client configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new client configuration builder.
    pub fn builder() -> ClientConfigBuilder {
        ClientConfigBuilder::default()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn client_config_builder() {
        let config = ClientConfig::builder()
            .with_navigation_timeout(Duration::from_secs(60))
            .with_element_timeout(Duration::from_secs(20))
            .with_extract_html(true)
            .with_extract_text(true)
            .with_user_agent("TestBot/1.0")
            .with_max_response_size(5_usize * 1024 * 1024)
            .with_capture_screenshots(true)
            .build()
            .unwrap();

        assert_eq!(config.navigation_timeout, Duration::from_secs(60));
        assert_eq!(config.element_timeout, Duration::from_secs(20));
        assert!(config.extract_html);
        assert!(config.extract_text);
        assert_eq!(config.user_agent, Some("TestBot/1.0".to_string()));
        assert_eq!(config.max_response_size, 5_usize * 1024 * 1024);
        assert!(config.capture_screenshots);
    }

    #[test]
    fn client_config_builder_validation() {
        // Test zero navigation timeout
        let result = ClientConfig::builder()
            .with_navigation_timeout(Duration::from_secs(0))
            .build();
        assert!(result.is_err());

        // Test zero element timeout
        let result = ClientConfig::builder()
            .with_element_timeout(Duration::from_secs(0))
            .build();
        assert!(result.is_err());

        // Test zero max response size
        let result = ClientConfig::builder()
            .with_max_response_size(0_usize)
            .build();
        assert!(result.is_err());

        // Test empty user agent
        let result = ClientConfig::builder().with_user_agent("").build();
        assert!(result.is_err());

        // Test valid configuration
        let result = ClientConfig::builder()
            .with_navigation_timeout(Duration::from_secs(30))
            .with_element_timeout(Duration::from_secs(10))
            .with_max_response_size(1024_usize)
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn client_config_default() {
        let config = ClientConfig::default();
        assert_eq!(config.navigation_timeout, Duration::from_secs(30));
        assert_eq!(config.element_timeout, Duration::from_secs(10));
        assert!(config.extract_html);
        assert!(!config.extract_text);
        assert!(config.enable_javascript);
        assert!(config.wait_for_load);
        assert_eq!(config.user_agent, None);
        assert_eq!(config.max_response_size, 10_usize * 1024 * 1024);
        assert!(!config.capture_screenshots);
    }
}
