//! WebDriver capabilities utilities and helpers.

use std::collections::HashMap;

use serde_json::{Value, json};

/// Standard WebDriver capability keys.
pub mod keys {
    /// Browser name capability
    pub const BROWSER_NAME: &str = "browserName";
    /// Browser version capability
    pub const BROWSER_VERSION: &str = "browserVersion";
    /// Platform name capability
    pub const PLATFORM_NAME: &str = "platformName";
    /// Accept insecure certificates capability
    pub const ACCEPT_INSECURE_CERTS: &str = "acceptInsecureCerts";
    /// Page load strategy capability
    pub const PAGE_LOAD_STRATEGY: &str = "pageLoadStrategy";
    /// Proxy configuration capability
    pub const PROXY: &str = "proxy";
    /// Window rect capability
    pub const SET_WINDOW_RECT: &str = "setWindowRect";
    /// Timeouts capability
    pub const TIMEOUTS: &str = "timeouts";
    /// Strict file interactability capability
    pub const STRICT_FILE_INTERACTABILITY: &str = "strictFileInteractability";
    /// Unhandled prompt behavior capability
    pub const UNHANDLED_PROMPT_BEHAVIOR: &str = "unhandledPromptBehavior";

    // Browser-specific capability keys
    /// Chrome options capability
    pub const CHROME_OPTIONS: &str = "goog:chromeOptions";
    /// Firefox options capability
    pub const FIREFOX_OPTIONS: &str = "moz:firefoxOptions";
    /// Edge options capability
    pub const EDGE_OPTIONS: &str = "ms:edgeOptions";
    /// Safari options capability
    pub const SAFARI_OPTIONS: &str = "webkit:WebRTCConfiguration";
}

/// Page load strategy values.
pub mod page_load_strategy {
    /// Wait for the page to be completely loaded
    pub const NORMAL: &str = "normal";
    /// Wait for the initial page load to complete
    pub const EAGER: &str = "eager";
    /// Don't wait for page load
    pub const NONE: &str = "none";
}

/// Unhandled prompt behavior values.
pub mod unhandled_prompt_behavior {
    /// Dismiss the prompt
    pub const DISMISS: &str = "dismiss";
    /// Accept the prompt
    pub const ACCEPT: &str = "accept";
    /// Dismiss and notify
    pub const DISMISS_AND_NOTIFY: &str = "dismiss and notify";
    /// Accept and notify
    pub const ACCEPT_AND_NOTIFY: &str = "accept and notify";
    /// Ignore the prompt
    pub const IGNORE: &str = "ignore";
}

/// Helper functions for building WebDriver capabilities.
pub struct CapabilitiesBuilder {
    capabilities: HashMap<String, Value>,
}

impl CapabilitiesBuilder {
    /// Creates a new capabilities builder.
    pub fn new() -> Self {
        Self {
            capabilities: HashMap::new(),
        }
    }

    /// Sets the browser name.
    pub fn browser_name(mut self, name: impl Into<String>) -> Self {
        self.capabilities
            .insert(keys::BROWSER_NAME.to_string(), json!(name.into()));
        self
    }

    /// Sets the browser version.
    pub fn browser_version(mut self, version: impl Into<String>) -> Self {
        self.capabilities
            .insert(keys::BROWSER_VERSION.to_string(), json!(version.into()));
        self
    }

    /// Sets the platform name.
    pub fn platform_name(mut self, platform: impl Into<String>) -> Self {
        self.capabilities
            .insert(keys::PLATFORM_NAME.to_string(), json!(platform.into()));
        self
    }

    /// Sets whether to accept insecure certificates.
    pub fn accept_insecure_certs(mut self, accept: bool) -> Self {
        self.capabilities
            .insert(keys::ACCEPT_INSECURE_CERTS.to_string(), json!(accept));
        self
    }

    /// Sets the page load strategy.
    pub fn page_load_strategy(mut self, strategy: impl Into<String>) -> Self {
        self.capabilities
            .insert(keys::PAGE_LOAD_STRATEGY.to_string(), json!(strategy.into()));
        self
    }

    /// Sets whether the browser supports setting window rect.
    pub fn set_window_rect(mut self, supported: bool) -> Self {
        self.capabilities
            .insert(keys::SET_WINDOW_RECT.to_string(), json!(supported));
        self
    }

    /// Sets strict file interactability.
    pub fn strict_file_interactability(mut self, strict: bool) -> Self {
        self.capabilities
            .insert(keys::STRICT_FILE_INTERACTABILITY.to_string(), json!(strict));
        self
    }

    /// Sets unhandled prompt behavior.
    pub fn unhandled_prompt_behavior(mut self, behavior: impl Into<String>) -> Self {
        self.capabilities.insert(
            keys::UNHANDLED_PROMPT_BEHAVIOR.to_string(),
            json!(behavior.into()),
        );
        self
    }

    /// Sets timeouts configuration.
    pub fn timeouts(
        mut self,
        implicit: Option<u64>,
        page_load: Option<u64>,
        script: Option<u64>,
    ) -> Self {
        let mut timeouts = serde_json::Map::new();

        if let Some(implicit_ms) = implicit {
            timeouts.insert("implicit".to_string(), json!(implicit_ms));
        }

        if let Some(page_load_ms) = page_load {
            timeouts.insert("pageLoad".to_string(), json!(page_load_ms));
        }

        if let Some(script_ms) = script {
            timeouts.insert("script".to_string(), json!(script_ms));
        }

        self.capabilities
            .insert(keys::TIMEOUTS.to_string(), json!(timeouts));
        self
    }

    /// Sets proxy configuration.
    pub fn proxy(mut self, proxy_config: Value) -> Self {
        self.capabilities
            .insert(keys::PROXY.to_string(), proxy_config);
        self
    }

    /// Sets a custom capability.
    pub fn capability(mut self, key: impl Into<String>, value: Value) -> Self {
        self.capabilities.insert(key.into(), value);
        self
    }

    /// Adds multiple capabilities.
    pub fn capabilities(mut self, caps: HashMap<String, Value>) -> Self {
        self.capabilities.extend(caps);
        self
    }

    /// Builds the capabilities map.
    pub fn build(self) -> HashMap<String, Value> {
        self.capabilities
    }
}

impl Default for CapabilitiesBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Merges two capability maps, with the second map taking precedence.
pub fn merge_capabilities(
    base: HashMap<String, Value>,
    override_caps: HashMap<String, Value>,
) -> HashMap<String, Value> {
    let mut merged = base;

    for (key, value) in override_caps {
        // Special handling for nested objects (like browser options)
        if let (Some(base_value), Value::Object(override_obj)) = (merged.get(&key), &value)
            && let Value::Object(base_obj) = base_value {
                let mut merged_obj = base_obj.clone();
                merged_obj.extend(override_obj.clone());
                merged.insert(key, Value::Object(merged_obj));
                continue;
            }

        merged.insert(key, value);
    }

    merged
}

/// Validates WebDriver capabilities for common issues.
pub fn validate_capabilities(capabilities: &HashMap<String, Value>) -> Result<(), String> {
    // Check for required browser name
    if !capabilities.contains_key(keys::BROWSER_NAME) {
        return Err("Browser name is required".to_string());
    }

    // Validate page load strategy
    if let Some(strategy) = capabilities.get(keys::PAGE_LOAD_STRATEGY)
        && let Some(strategy_str) = strategy.as_str() {
            match strategy_str {
                page_load_strategy::NORMAL
                | page_load_strategy::EAGER
                | page_load_strategy::NONE => {}
                _ => return Err(format!("Invalid page load strategy: {}", strategy_str)),
            }
        }

    // Validate unhandled prompt behavior
    if let Some(behavior) = capabilities.get(keys::UNHANDLED_PROMPT_BEHAVIOR)
        && let Some(behavior_str) = behavior.as_str() {
            match behavior_str {
                unhandled_prompt_behavior::DISMISS
                | unhandled_prompt_behavior::ACCEPT
                | unhandled_prompt_behavior::DISMISS_AND_NOTIFY
                | unhandled_prompt_behavior::ACCEPT_AND_NOTIFY
                | unhandled_prompt_behavior::IGNORE => {}
                _ => {
                    return Err(format!(
                        "Invalid unhandled prompt behavior: {}",
                        behavior_str
                    ));
                }
            }
        }

    // Validate timeouts format
    if let Some(timeouts) = capabilities.get(keys::TIMEOUTS)
        && !timeouts.is_object() {
            return Err("Timeouts must be an object".to_string());
        }

    Ok(())
}

/// Creates a proxy configuration for WebDriver.
pub fn create_proxy(proxy_type: &str, proxy_url: Option<&str>) -> Value {
    let mut proxy = serde_json::Map::new();
    proxy.insert("proxyType".to_string(), json!(proxy_type));

    if let Some(url) = proxy_url {
        match proxy_type.to_lowercase().as_str() {
            "manual" => {
                proxy.insert("httpProxy".to_string(), json!(url));
                proxy.insert("sslProxy".to_string(), json!(url));
            }
            "pac" => {
                proxy.insert("proxyAutoconfigUrl".to_string(), json!(url));
            }
            _ => {}
        }
    }

    Value::Object(proxy)
}

/// Creates Chrome-specific arguments list.
pub fn chrome_args(headless: bool, disable_images: bool, window_size: Option<&str>) -> Vec<Value> {
    let mut args = vec![
        json!("--no-sandbox"),
        json!("--disable-dev-shm-usage"),
        json!("--disable-gpu"),
        json!("--disable-extensions"),
        json!("--disable-background-timer-throttling"),
        json!("--disable-backgrounding-occluded-windows"),
        json!("--disable-renderer-backgrounding"),
    ];

    if headless {
        args.push(json!("--headless"));
        args.push(json!(window_size.unwrap_or("--window-size=1920,1080")));
    }

    if disable_images {
        args.push(json!("--disable-images"));
    }

    args
}

/// Creates Firefox-specific arguments list.
pub fn firefox_args(headless: bool, window_size: Option<&str>) -> Vec<Value> {
    let mut args = vec![json!("--no-sandbox"), json!("--disable-dev-shm-usage")];

    if headless {
        args.push(json!("--headless"));
        if let Some(size) = window_size {
            args.push(json!(format!(
                "--width={}",
                size.split('x').next().unwrap_or("1920")
            )));
            args.push(json!(format!(
                "--height={}",
                size.split('x').nth(1).unwrap_or("1080")
            )));
        }
    }

    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_builder() {
        let caps = CapabilitiesBuilder::new()
            .browser_name("chrome")
            .browser_version("latest")
            .accept_insecure_certs(true)
            .page_load_strategy(page_load_strategy::NORMAL)
            .timeouts(Some(1000), Some(30000), Some(30000))
            .build();

        assert_eq!(caps[keys::BROWSER_NAME], json!("chrome"));
        assert_eq!(caps[keys::BROWSER_VERSION], json!("latest"));
        assert_eq!(caps[keys::ACCEPT_INSECURE_CERTS], json!(true));
        assert_eq!(caps[keys::PAGE_LOAD_STRATEGY], json!("normal"));
        assert!(caps.contains_key(keys::TIMEOUTS));
    }

    #[test]
    fn merge_capabilities_basic() {
        let base = HashMap::from([
            ("key1".to_string(), json!("value1")),
            ("key2".to_string(), json!("value2")),
        ]);

        let override_caps = HashMap::from([
            ("key2".to_string(), json!("override2")),
            ("key3".to_string(), json!("value3")),
        ]);

        let merged = merge_capabilities(base, override_caps);

        assert_eq!(merged["key1"], json!("value1"));
        assert_eq!(merged["key2"], json!("override2"));
        assert_eq!(merged["key3"], json!("value3"));
    }

    #[test]
    fn validate_capabilities_success() {
        let caps = HashMap::from([
            (keys::BROWSER_NAME.to_string(), json!("chrome")),
            (
                keys::PAGE_LOAD_STRATEGY.to_string(),
                json!(page_load_strategy::NORMAL),
            ),
        ]);

        assert!(validate_capabilities(&caps).is_ok());
    }

    #[test]
    fn validate_capabilities_missing_browser() {
        let caps = HashMap::new();
        let result = validate_capabilities(&caps);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Browser name is required"));
    }

    #[test]
    fn validate_capabilities_invalid_strategy() {
        let caps = HashMap::from([
            (keys::BROWSER_NAME.to_string(), json!("chrome")),
            (keys::PAGE_LOAD_STRATEGY.to_string(), json!("invalid")),
        ]);

        let result = validate_capabilities(&caps);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid page load strategy"));
    }

    #[test]
    fn create_proxy_manual() {
        let proxy = create_proxy("manual", Some("proxy.example.com:8080"));

        assert_eq!(proxy["proxyType"], json!("manual"));
        assert_eq!(proxy["httpProxy"], json!("proxy.example.com:8080"));
        assert_eq!(proxy["sslProxy"], json!("proxy.example.com:8080"));
    }

    #[test]
    fn chrome_args_headless() {
        let args = chrome_args(true, true, Some("--window-size=1280,720"));

        let args_strings: Vec<String> = args
            .iter()
            .map(|v| v.as_str().unwrap_or("").to_string())
            .collect();

        assert!(args_strings.contains(&"--headless".to_string()));
        assert!(args_strings.contains(&"--disable-images".to_string()));
        assert!(args_strings.contains(&"--window-size=1280,720".to_string()));
    }

    #[test]
    fn firefox_args_headless() {
        let args = firefox_args(true, Some("1280x720"));

        let args_strings: Vec<String> = args
            .iter()
            .map(|v| v.as_str().unwrap_or("").to_string())
            .collect();

        assert!(args_strings.contains(&"--headless".to_string()));
        assert!(args_strings.contains(&"--width=1280".to_string()));
        assert!(args_strings.contains(&"--height=720".to_string()));
    }
}
