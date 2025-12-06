use std::fmt;
use std::ops::{Deref, DerefMut};

use bytes::Bytes;
use deadpool::managed::Object;
use http::StatusCode;
use spire_core::backend::Client;
use spire_core::context::{Body, Request, Response};
use spire_core::{Error, Result};
use thirtyfour::prelude::*;

use crate::pool::BrowserBehaviorConfig;
use crate::pool::manager::BrowserManager;

/// Browser connection that provides direct access to a pooled WebDriver instance.
///
/// This connection implements `Deref` and `DerefMut` to provide transparent access
/// to the underlying WebDriver, allowing you to call any WebDriver method directly
/// on the connection instance.
///
/// # Examples
///
/// ```ignore
/// use spire_thirtyfour::BrowserConnection;
///
/// let connection = BrowserConnection::from_pooled(pooled_webdriver);
///
/// // You can call WebDriver methods directly on the connection
/// connection.goto("https://example.com").await?;
/// let title = connection.title().await?;
/// ```
pub struct BrowserConnection {
    /// The pooled WebDriver instance
    driver: Object<BrowserManager>,
    /// Client configuration for request processing
    config: BrowserBehaviorConfig,
}

impl BrowserConnection {
    /// Creates a new BrowserConnection from a pooled WebDriver instance.
    ///
    /// # Arguments
    ///
    /// * `driver` - A pooled WebDriver instance from the browser pool
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let pooled_driver = pool.get().await?;
    /// let connection = BrowserConnection::from_pooled(pooled_driver);
    /// ```
    pub fn from_pooled(driver: Object<BrowserManager>) -> Self {
        Self {
            driver,
            config: BrowserBehaviorConfig::default(),
        }
    }

    /// Creates a new BrowserConnection with custom client configuration.
    ///
    /// # Arguments
    ///
    /// * `driver` - A pooled WebDriver instance from the browser pool
    /// * `config` - Client configuration for request processing behavior
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let config = ClientConfig::builder()
    ///     .with_extract_text(true)
    ///     .with_user_agent("MyBot/1.0")
    ///     .build()?;
    ///
    /// let connection = BrowserConnection::from_pooled_with_config(pooled_driver, config);
    /// ```
    pub fn from_pooled_with_config(
        driver: Object<BrowserManager>,
        config: BrowserBehaviorConfig,
    ) -> Self {
        Self { driver, config }
    }

    /// Returns a reference to the client configuration.
    pub fn config(&self) -> &BrowserBehaviorConfig {
        &self.config
    }

    /// Updates the client configuration.
    pub fn with_config(mut self, config: BrowserBehaviorConfig) -> Self {
        self.config = config;
        self
    }
}

impl Deref for BrowserConnection {
    type Target = WebDriver;

    fn deref(&self) -> &Self::Target {
        &self.driver
    }
}

impl DerefMut for BrowserConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.driver
    }
}

impl fmt::Debug for BrowserConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserConnection")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

#[spire_core::async_trait]
impl Client for BrowserConnection {
    /// Resolves an HTTP request using browser automation.
    ///
    /// This method performs a simplified browser automation workflow:
    /// 1. Navigates to the request URL using the underlying WebDriver
    /// 2. Extracts the page content based on configuration
    /// 3. Returns an HTTP response with the extracted content
    ///
    /// The connection can be used like a regular WebDriver through Deref,
    /// so you have full control over the browser behavior.
    async fn resolve(mut self, req: Request) -> Result<Response> {
        // Extract URL from request
        let uri = req.uri().clone();
        let url = uri.to_string();

        // Validate URL
        if url.is_empty() {
            return Err(Error::new(
                spire_core::ErrorKind::Backend,
                "Empty URL provided",
            ));
        }

        // Set user agent if configured
        if let Some(user_agent) = &self.config.user_agent {
            let script = format!(
                r#"Object.defineProperty(navigator, 'userAgent', {{
                    get: function () {{ return '{}'; }}
                }});"#,
                user_agent.replace('\'', "\\'")
            );

            if let Err(e) = self.execute(script, vec![]).await {
                eprintln!("Failed to set user agent: {}", e);
                // Continue anyway - this is not critical
            }
        }

        // Navigate to the URL
        self.goto(&url).await.map_err(|e| {
            Error::new(
                spire_core::ErrorKind::Backend,
                format!("Failed to navigate to {}: {}", url, e),
            )
        })?;

        // Wait for page load if configured
        if self.config.wait_for_load {
            // Simple wait for document ready state
            let script = r#"
                return new Promise((resolve) => {
                    if (document.readyState === 'complete') {
                        resolve();
                    } else {
                        window.addEventListener('load', resolve);
                    }
                });
            "#;

            let _ = tokio::time::timeout(self.config.element_timeout, self.execute(script, vec![]))
                .await;
        }

        // Extract content based on configuration
        let mut content = String::new();

        if self.config.extract_html {
            content = self.source().await.map_err(|e| {
                Error::new(
                    spire_core::ErrorKind::Backend,
                    format!("Failed to extract HTML: {}", e),
                )
            })?;
        } else if self.config.extract_text {
            let script = r#"
                return document.body ?
                    document.body.innerText || document.body.textContent || '' : '';
            "#;

            if let Ok(result) = self.execute(script, vec![]).await {
                content = result.json().as_str().unwrap_or("").to_string();
            }
        }

        // If no specific extraction was configured, default to HTML
        if content.is_empty() && !self.config.extract_html && !self.config.extract_text {
            content = self.source().await.unwrap_or_default();
        }

        // Limit response size if configured
        if content.len() > self.config.max_response_size {
            content.truncate(self.config.max_response_size);
        }

        // Build the response
        let body = Body::from(Bytes::from(content));
        let response = Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/html; charset=utf-8")
            .body(body)
            .map_err(|e| {
                Error::new(
                    spire_core::ErrorKind::Backend,
                    format!("Failed to build response: {}", e),
                )
            })?;

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_connection() {
        // We can't create a real connection without a pool in tests,
        // but we can test that the Debug impl compiles
        let config = BrowserBehaviorConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(!debug_str.is_empty());
    }

    #[test]
    fn test_config_management() {
        // Test ClientConfig creation and modification
        let mut config = BrowserBehaviorConfig::default();
        assert!(config.extract_html);
        assert!(!config.extract_text);

        config.extract_text = true;
        assert!(config.extract_text);

        // Test builder pattern
        let config2 = BrowserBehaviorConfig::builder()
            .with_extract_text(true)
            .with_user_agent("TestBot/1.0".to_string())
            .build()
            .expect("Should build successfully");

        assert!(config2.extract_text);
        assert_eq!(config2.user_agent, Some("TestBot/1.0".to_string()));
    }
}
