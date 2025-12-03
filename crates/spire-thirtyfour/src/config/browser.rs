//! Browser type definitions and configurations.

use std::collections::HashMap;

use derive_more::Display;
use serde_json::{Value, json};

/// Supported browser types for WebDriver automation.
#[derive(Debug, Clone, PartialEq, Eq, Display)]
#[derive(Default)]
pub enum BrowserType {
    /// Google Chrome browser
    #[display("Chrome")]
    #[default]
    Chrome,
    /// Mozilla Firefox browser
    #[display("Firefox")]
    Firefox,
    /// Microsoft Edge browser
    #[display("Edge")]
    Edge,
    /// Apple Safari browser (macOS only)
    #[display("Safari")]
    Safari,
    /// Custom browser with explicit capabilities
    #[display("Custom({})", name)]
    Custom {
        /// Browser name
        name: String,
        /// Custom capabilities
        capabilities: HashMap<String, Value>,
    },
}

impl BrowserType {
    /// Returns the WebDriver browser name for this browser type.
    pub fn browser_name(&self) -> &str {
        match self {
            BrowserType::Chrome => "chrome",
            BrowserType::Firefox => "firefox",
            BrowserType::Edge => "MicrosoftEdge",
            BrowserType::Safari => "safari",
            BrowserType::Custom { name, .. } => name,
        }
    }

    /// Returns the default WebDriver capabilities for this browser type.
    pub fn default_capabilities(&self) -> HashMap<String, Value> {
        let mut caps = HashMap::new();

        match self {
            BrowserType::Chrome => {
                caps.insert("browserName".to_string(), json!("chrome"));
                caps.insert(
                    "goog:chromeOptions".to_string(),
                    json!({
                        "args": [
                            "--no-sandbox",
                            "--disable-dev-shm-usage",
                            "--disable-gpu",
                            "--disable-extensions",
                            "--disable-background-timer-throttling",
                            "--disable-backgrounding-occluded-windows",
                            "--disable-renderer-backgrounding"
                        ],
                        "prefs": {
                            "profile.default_content_setting_values.notifications": 2,
                            "profile.default_content_settings.popups": 0,
                            "profile.managed_default_content_settings.images": 2
                        }
                    }),
                );
            }
            BrowserType::Firefox => {
                caps.insert("browserName".to_string(), json!("firefox"));
                caps.insert(
                    "moz:firefoxOptions".to_string(),
                    json!({
                        "args": [
                            "--no-sandbox",
                            "--disable-dev-shm-usage"
                        ],
                        "prefs": {
                            "dom.webnotifications.enabled": false,
                            "dom.push.enabled": false,
                            "permissions.default.desktop-notification": 2,
                            "permissions.default.geo": 2
                        }
                    }),
                );
            }
            BrowserType::Edge => {
                caps.insert("browserName".to_string(), json!("MicrosoftEdge"));
                caps.insert(
                    "ms:edgeOptions".to_string(),
                    json!({
                        "args": [
                            "--no-sandbox",
                            "--disable-dev-shm-usage",
                            "--disable-gpu",
                            "--disable-extensions"
                        ]
                    }),
                );
            }
            BrowserType::Safari => {
                caps.insert("browserName".to_string(), json!("safari"));
                caps.insert(
                    "webkit:WebRTCConfiguration".to_string(),
                    json!({
                        "DisableInsecureMediaCapture": true
                    }),
                );
            }
            BrowserType::Custom { name, capabilities } => {
                caps.insert("browserName".to_string(), json!(name));
                caps.extend(capabilities.clone());
            }
        }

        // Common capabilities for all browsers
        caps.insert("acceptInsecureCerts".to_string(), json!(true));
        caps.insert("pageLoadStrategy".to_string(), json!("normal"));

        caps
    }

    /// Returns capabilities optimized for headless operation.
    pub fn headless_capabilities(&self) -> HashMap<String, Value> {
        let mut caps = self.default_capabilities();

        match self {
            BrowserType::Chrome => {
                if let Some(chrome_options) = caps.get_mut("goog:chromeOptions")
                    && let Some(args) = chrome_options.get_mut("args")
                    && let Some(args_array) = args.as_array_mut()
                {
                    args_array.push(json!("--headless"));
                    args_array.push(json!("--window-size=1920,1080"));
                }
            }
            BrowserType::Firefox => {
                if let Some(firefox_options) = caps.get_mut("moz:firefoxOptions")
                    && let Some(args) = firefox_options.get_mut("args")
                    && let Some(args_array) = args.as_array_mut()
                {
                    args_array.push(json!("--headless"));
                    args_array.push(json!("--width=1920"));
                    args_array.push(json!("--height=1080"));
                }
            }
            BrowserType::Edge => {
                if let Some(edge_options) = caps.get_mut("ms:edgeOptions")
                    && let Some(args) = edge_options.get_mut("args")
                    && let Some(args_array) = args.as_array_mut()
                {
                    args_array.push(json!("--headless"));
                    args_array.push(json!("--window-size=1920,1080"));
                }
            }
            BrowserType::Safari => {
                // Safari doesn't support headless mode in the same way
                // Add appropriate configurations if needed
            }
            BrowserType::Custom { .. } => {
                // Custom browsers should handle their own headless configuration
            }
        }

        caps
    }

    /// Returns capabilities optimized for performance (images disabled, etc.).
    pub fn performance_capabilities(&self) -> HashMap<String, Value> {
        let mut caps = self.default_capabilities();

        match self {
            BrowserType::Chrome => {
                if let Some(chrome_options) = caps.get_mut("goog:chromeOptions")
                    && let Some(prefs) = chrome_options.get_mut("prefs")
                    && let Some(prefs_obj) = prefs.as_object_mut()
                {
                    prefs_obj.insert(
                        "profile.managed_default_content_settings.images".to_string(),
                        json!(2),
                    );
                    prefs_obj.insert(
                        "profile.default_content_setting_values.plugins".to_string(),
                        json!(2),
                    );
                    prefs_obj.insert(
                        "profile.default_content_setting_values.media_stream".to_string(),
                        json!(2),
                    );
                }
            }
            BrowserType::Firefox => {
                if let Some(firefox_options) = caps.get_mut("moz:firefoxOptions")
                    && let Some(prefs) = firefox_options.get_mut("prefs")
                    && let Some(prefs_obj) = prefs.as_object_mut()
                {
                    prefs_obj.insert("permissions.default.image".to_string(), json!(2));
                    prefs_obj.insert("media.navigator.enabled".to_string(), json!(false));
                    prefs_obj.insert("media.peerconnection.enabled".to_string(), json!(false));
                }
            }
            BrowserType::Edge => {
                // Similar optimizations for Edge
                if let Some(edge_options) = caps.get_mut("ms:edgeOptions")
                    && let Some(args) = edge_options.get_mut("args")
                    && let Some(args_array) = args.as_array_mut()
                {
                    args_array.push(json!("--disable-images"));
                    args_array.push(json!("--disable-javascript"));
                    args_array.push(json!("--disable-plugins"));
                }
            }
            BrowserType::Safari | BrowserType::Custom { .. } => {
                // Less granular control for Safari and custom browsers
            }
        }

        caps
    }

    /// Creates a Chrome browser configuration.
    pub fn chrome() -> Self {
        BrowserType::Chrome
    }

    /// Creates a Firefox browser configuration.
    pub fn firefox() -> Self {
        BrowserType::Firefox
    }

    /// Creates an Edge browser configuration.
    pub fn edge() -> Self {
        BrowserType::Edge
    }

    /// Creates a Safari browser configuration.
    pub fn safari() -> Self {
        BrowserType::Safari
    }

    /// Creates a custom browser configuration.
    pub fn custom(name: impl Into<String>) -> Self {
        BrowserType::Custom {
            name: name.into(),
            capabilities: HashMap::new(),
        }
    }

    /// Creates a custom browser configuration with initial capabilities.
    pub fn custom_with_capabilities(
        name: impl Into<String>,
        capabilities: HashMap<String, Value>,
    ) -> Self {
        BrowserType::Custom {
            name: name.into(),
            capabilities,
        }
    }

    /// Adds a capability to a custom browser type.
    pub fn with_capability(mut self, key: impl Into<String>, value: Value) -> Self {
        match &mut self {
            BrowserType::Custom { capabilities, .. } => {
                capabilities.insert(key.into(), value);
            }
            _ => {
                // Convert to custom browser type
                let name = self.browser_name().to_string();
                let mut caps = self.default_capabilities();
                caps.insert(key.into(), value);
                self = BrowserType::Custom {
                    name,
                    capabilities: caps,
                };
            }
        }
        self
    }

    /// Returns whether this browser type supports headless operation.
    pub fn supports_headless(&self) -> bool {
        match self {
            BrowserType::Chrome | BrowserType::Firefox | BrowserType::Edge => true,
            BrowserType::Safari => false, // Safari has limited headless support
            BrowserType::Custom { .. } => true, // Assume custom browsers can handle headless
        }
    }
}

// Custom From implementation for string parsing with browser name matching
impl From<&str> for BrowserType {
    fn from(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "chrome" | "google-chrome" => BrowserType::Chrome,
            "firefox" | "mozilla-firefox" => BrowserType::Firefox,
            "edge" | "microsoft-edge" | "microsoftedge" => BrowserType::Edge,
            "safari" | "apple-safari" => BrowserType::Safari,
            name => BrowserType::custom(name),
        }
    }
}

impl From<String> for BrowserType {
    fn from(value: String) -> Self {
        BrowserType::from(value.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_type_display() {
        assert_eq!(BrowserType::Chrome.to_string(), "Chrome");
        assert_eq!(BrowserType::Firefox.to_string(), "Firefox");
        assert_eq!(BrowserType::Edge.to_string(), "Edge");
        assert_eq!(BrowserType::Safari.to_string(), "Safari");
        assert_eq!(BrowserType::custom("custom").to_string(), "Custom(custom)");
    }

    #[test]
    fn browser_type_from_str() {
        assert_eq!(BrowserType::from("chrome"), BrowserType::Chrome);
        assert_eq!(BrowserType::from("firefox"), BrowserType::Firefox);
        assert_eq!(BrowserType::from("edge"), BrowserType::Edge);
        assert_eq!(BrowserType::from("safari"), BrowserType::Safari);

        match BrowserType::from("unknown") {
            BrowserType::Custom { name, .. } => assert_eq!(name, "unknown"),
            _ => panic!("Expected Custom browser type"),
        }
    }

    #[test]
    fn browser_name() {
        assert_eq!(BrowserType::Chrome.browser_name(), "chrome");
        assert_eq!(BrowserType::Firefox.browser_name(), "firefox");
        assert_eq!(BrowserType::Edge.browser_name(), "MicrosoftEdge");
        assert_eq!(BrowserType::Safari.browser_name(), "safari");
    }

    #[test]
    fn default_capabilities() {
        let chrome_caps = BrowserType::Chrome.default_capabilities();
        assert!(chrome_caps.contains_key("browserName"));
        assert!(chrome_caps.contains_key("goog:chromeOptions"));
        assert!(chrome_caps.contains_key("acceptInsecureCerts"));

        let firefox_caps = BrowserType::Firefox.default_capabilities();
        assert!(firefox_caps.contains_key("browserName"));
        assert!(firefox_caps.contains_key("moz:firefoxOptions"));
    }

    #[test]
    fn headless_capabilities() {
        let chrome_headless = BrowserType::Chrome.headless_capabilities();
        let chrome_options = &chrome_headless["goog:chromeOptions"];
        let args = &chrome_options["args"];
        let args_str = args.to_string();
        assert!(args_str.contains("--headless"));
    }

    #[test]
    fn supports_headless() {
        assert!(BrowserType::Chrome.supports_headless());
        assert!(BrowserType::Firefox.supports_headless());
        assert!(BrowserType::Edge.supports_headless());
        assert!(!BrowserType::Safari.supports_headless());
        assert!(BrowserType::custom("test").supports_headless());
    }

    #[test]
    fn with_capability() {
        let browser = BrowserType::Chrome.with_capability("test", json!("value"));
        match browser {
            BrowserType::Custom { capabilities, .. } => {
                assert!(capabilities.contains_key("test"));
                assert_eq!(capabilities["test"], json!("value"));
            }
            _ => panic!("Expected Custom browser type after adding capability"),
        }
    }
}
