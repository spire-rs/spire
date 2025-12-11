//! Browser configuration types.

use std::time::Duration;

use derive_builder::Builder;

/// Configuration for browser behavior.
#[derive(Debug, Clone, Builder)]
#[builder(
    name = "BrowserBehaviorConfigBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(validate = "Self::validate_config")
)]
pub struct BrowserBehaviorConfig {
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

impl BrowserBehaviorConfigBuilder {
    fn validate_config(&self) -> Result<(), String> {
        if let Some(timeout) = &self.navigation_timeout
            && timeout.as_secs() == 0
        {
            return Err("Navigation timeout must be greater than 0".to_string());
        }

        if let Some(timeout) = &self.element_timeout
            && timeout.as_secs() == 0
        {
            return Err("Element timeout must be greater than 0".to_string());
        }

        if let Some(size) = &self.max_response_size
            && *size == 0
        {
            return Err("Max response size must be greater than 0".to_string());
        }

        if let Some(user_agent) = &self.user_agent
            && let Some(ua) = user_agent
            && ua.trim().is_empty()
        {
            return Err("User agent cannot be empty".to_string());
        }

        Ok(())
    }
}

impl Default for BrowserBehaviorConfig {
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

impl BrowserBehaviorConfig {
    /// Creates a new browser behavior configuration with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new browser configuration builder.
    pub fn builder() -> BrowserBehaviorConfigBuilder {
        BrowserBehaviorConfigBuilder::default()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn browser_behavior_config_builder() {
        let config = BrowserBehaviorConfig::builder()
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
    fn browser_behavior_config_builder_validation() {
        // Test zero navigation timeout
        let result = BrowserBehaviorConfig::builder()
            .with_navigation_timeout(Duration::from_secs(0))
            .build();
        assert!(result.is_err());

        // Test zero element timeout
        let result = BrowserBehaviorConfig::builder()
            .with_element_timeout(Duration::from_secs(0))
            .build();
        assert!(result.is_err());

        // Test zero max response size
        let result = BrowserBehaviorConfig::builder()
            .with_max_response_size(0_usize)
            .build();
        assert!(result.is_err());

        // Test empty user agent
        let result = BrowserBehaviorConfig::builder().with_user_agent("").build();
        assert!(result.is_err());

        // Test valid configuration
        let result = BrowserBehaviorConfig::builder()
            .with_navigation_timeout(Duration::from_secs(30))
            .with_element_timeout(Duration::from_secs(10))
            .with_max_response_size(1024_usize)
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn browser_behavior_config_default() {
        let config = BrowserBehaviorConfig::default();
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
