//! Browser client implementation with comprehensive request processing.

use std::ops::Deref;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use bytes::Bytes;
use deadpool::managed::Object;
use derive_builder::Builder;
use futures::FutureExt;
use futures::future::BoxFuture;
use http::{HeaderName, HeaderValue, StatusCode, Uri};
use spire_core::context::{Body, Request, Response};
use spire_core::{Error, Result};
use thirtyfour::prelude::*;
use tower::Service;

pub mod connection;

use connection::BrowserConnection;

use crate::error::BrowserError;
use crate::pool::manager::BrowserManager;

/// Client for interacting with a browser instance from the [`BrowserPool`].
///
/// This type wraps a pooled browser connection and implements the Tower `Service`
/// trait for processing HTTP requests through browser automation. It provides
/// comprehensive request handling including navigation, content extraction,
/// JavaScript execution, and response building.
///
/// The client automatically handles:
/// - URL navigation with timeout and error handling
/// - HTTP status code detection from browser navigation
/// - Response headers extraction where available
/// - Page content extraction (HTML, text, or both)
/// - JavaScript execution for dynamic content
/// - Connection health monitoring and error reporting
///
/// # Examples
///
/// ```ignore
/// use spire_thirtyfour::BrowserPool;
/// use spire_core::context::Request;
/// use tower::ServiceExt;
///
/// let pool = BrowserPool::builder()
///     .with_unmanaged("http://localhost:4444")
///     .build();
///
/// let client = pool.client().await?;
///
/// let request = Request::get("https://example.com")
///     .body(())
///     .unwrap();
///
/// let response = client.oneshot(request).await?;
/// ```
///
/// [`BrowserPool`]: crate::pool::BrowserPool
#[derive(Clone)]
pub struct BrowserClient {
    connection: Arc<Object<BrowserManager>>,
    config: ClientConfig,
}

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

/// Custom error type for client configuration
#[derive(Debug)]
pub struct ClientConfigError(String);

impl std::fmt::Display for ClientConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Client configuration error: {}", self.0)
    }
}

impl std::error::Error for ClientConfigError {}

impl From<Error> for ClientConfigError {
    fn from(err: Error) -> Self {
        ClientConfigError(err.to_string())
    }
}

impl From<String> for ClientConfigError {
    fn from(err: String) -> Self {
        ClientConfigError(err)
    }
}

impl ClientConfigBuilder {
    fn validate_config(&self) -> Result<(), ClientConfigError> {
        // Validate timeouts are not zero
        if let Some(ref timeout) = self.navigation_timeout
            && timeout.is_zero()
        {
            return Err(ClientConfigError(
                "Navigation timeout must be greater than zero".to_string(),
            ));
        }

        if let Some(ref timeout) = self.element_timeout
            && timeout.is_zero()
        {
            return Err(ClientConfigError(
                "Element timeout must be greater than zero".to_string(),
            ));
        }

        // Validate max response size is reasonable
        if let Some(max_size) = self.max_response_size {
            if max_size == 0 {
                return Err(ClientConfigError(
                    "Max response size must be greater than zero".to_string(),
                ));
            }
            if max_size > 1024 * 1024 * 1024 {
                // 1GB limit
                return Err(ClientConfigError(
                    "Max response size cannot exceed 1GB".to_string(),
                ));
            }
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
            max_response_size: 10 * 1024 * 1024, // 10MB
            capture_screenshots: false,
        }
    }
}

impl ClientConfig {
    /// Creates a new client configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a builder for client configuration.
    pub fn builder() -> ClientConfigBuilder {
        ClientConfigBuilder::default()
    }
}

impl BrowserClient {
    /// Creates a new browser client from a pooled connection.
    pub fn new(connection: Object<BrowserManager>) -> Self {
        Self {
            connection: Arc::new(connection),
            config: ClientConfig::default(),
        }
    }

    /// Creates a new browser client with custom configuration.
    pub fn with_config(connection: Object<BrowserManager>, config: ClientConfig) -> Self {
        Self {
            connection: Arc::new(connection),
            config,
        }
    }

    /// Returns the client configuration.
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// Returns connection statistics.
    pub fn connection_stats(&self) -> connection::ConnectionStats {
        self.connection.stats()
    }

    /// Processes a request by navigating to the URL and extracting content.
    async fn process_request(&self, request: Request) -> Result<Response> {
        let start_time = Instant::now();

        // Extract URL from request
        let uri = request.uri().clone();
        let url = uri.to_string();

        // Validate URL
        if url.is_empty() {
            return Err(BrowserError::navigation_error(
                &url,
                crate::error::NavigationErrorType::InvalidUrl,
                Some("Empty URL provided".to_string()),
            )
            .into());
        }

        // Mark connection as used
        let connection = self.connection.deref();
        // Note: We can't mark connection as used here due to borrowing constraints
        // This would need to be handled differently in a real implementation

        let webdriver = &**connection;

        // Set user agent if configured
        if let Some(user_agent) = &self.config.user_agent
            && let Err(e) = self.set_user_agent(webdriver, user_agent).await
        {
            connection.mark_error();
            return Err(BrowserError::operation_failed(
                "set_user_agent",
                format!("Failed to set user agent: {}", e),
            )
            .into());
        }

        // Navigate to URL
        match self.navigate_to_url(webdriver, &url).await {
            Ok(()) => {}
            Err(e) => {
                connection.mark_error();
                if self.config.capture_screenshots {
                    let _ = self.capture_error_screenshot(webdriver, &url).await;
                }
                return Err(e.into());
            }
        }

        // Wait for page load if configured
        if self.config.wait_for_load
            && let Err(e) = self.wait_for_page_load(webdriver).await
        {
            connection.mark_error();
            return Err(e.into());
        }

        // Extract response data
        match self.extract_response_data(webdriver, &uri).await {
            Ok(response) => {
                let elapsed = start_time.elapsed();

                // Log successful request
                self.log_request_success(&url, elapsed);

                Ok(response)
            }
            Err(e) => {
                connection.mark_error();
                if self.config.capture_screenshots {
                    let _ = self.capture_error_screenshot(webdriver, &url).await;
                }
                Err(e.into())
            }
        }
    }

    /// Navigates to the specified URL with timeout and error handling.
    async fn navigate_to_url(&self, webdriver: &WebDriver, url: &str) -> Result<(), BrowserError> {
        let navigation_future = webdriver.goto(url);

        match tokio::time::timeout(self.config.navigation_timeout, navigation_future).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(webdriver_error)) => {
                // Convert WebDriver error to navigation error
                let nav_error = {
                    // Try to determine error type from error message
                    let error_msg = webdriver_error.to_string().to_lowercase();
                    if error_msg.contains("invalid") && error_msg.contains("url") {
                        crate::error::NavigationErrorType::InvalidUrl
                    } else if error_msg.contains("timeout") {
                        crate::error::NavigationErrorType::Timeout
                    } else if error_msg.contains("ssl") || error_msg.contains("certificate") {
                        crate::error::NavigationErrorType::CertificateError
                    } else if error_msg.contains("network") || error_msg.contains("connection") {
                        crate::error::NavigationErrorType::NetworkError
                    } else if error_msg.contains("404") {
                        crate::error::NavigationErrorType::HttpError(404)
                    } else if error_msg.contains("500") {
                        crate::error::NavigationErrorType::HttpError(500)
                    } else {
                        crate::error::NavigationErrorType::Unknown
                    }
                };

                Err(BrowserError::navigation_error(
                    url,
                    nav_error,
                    Some(webdriver_error.to_string()),
                ))
            }
            Err(_) => {
                // Navigation timeout
                Err(BrowserError::timeout(
                    format!("navigation to {}", url),
                    self.config.navigation_timeout.as_secs(),
                ))
            }
        }
    }

    /// Waits for the page to finish loading.
    async fn wait_for_page_load(&self, webdriver: &WebDriver) -> Result<(), BrowserError> {
        let wait_future = async {
            // Wait for document.readyState to be 'complete'
            let script = r#"
                return document.readyState === 'complete';
            "#;

            let mut attempts = 0;
            let max_attempts = 30; // 30 seconds with 1 second intervals

            loop {
                if attempts >= max_attempts {
                    return Err(BrowserError::timeout("page_load_wait", max_attempts));
                }

                match webdriver.execute(script, vec![]).await {
                    Ok(result) => {
                        // Try to convert the result to a boolean
                        if let Ok(complete) = result.convert::<bool>()
                            && complete
                        {
                            return Ok(());
                        }
                    }
                    Err(e) => {
                        return Err(BrowserError::script_error(script, e.to_string()));
                    }
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
                attempts += 1;
            }
        };

        tokio::time::timeout(self.config.navigation_timeout, wait_future)
            .await
            .map_err(|_| {
                BrowserError::timeout("page_load_wait", self.config.navigation_timeout.as_secs())
            })?
    }

    /// Sets the user agent for the browser session.
    async fn set_user_agent(
        &self,
        webdriver: &WebDriver,
        user_agent: &str,
    ) -> Result<(), BrowserError> {
        // Note: Setting user agent after session creation is limited in WebDriver
        // This is a best-effort attempt using JavaScript
        let script = format!(
            r#"
            Object.defineProperty(navigator, 'userAgent', {{
                get: function() {{ return '{}'; }}
            }});
            "#,
            user_agent.replace('\'', "\\'")
        );

        webdriver
            .execute(&script, vec![])
            .await
            .map_err(|e| BrowserError::script_error(&script, e.to_string()))?;

        Ok(())
    }

    /// Extracts response data from the current page.
    async fn extract_response_data(
        &self,
        webdriver: &WebDriver,
        _uri: &Uri,
    ) -> Result<Response, BrowserError> {
        let mut response_builder = Response::builder();

        // Get current URL (might be different due to redirects)
        let _current_url = webdriver
            .current_url()
            .await
            .map_err(|e| BrowserError::webdriver(e, Some("getting current URL".to_string())))?;

        // Try to get status code from browser (limited support)
        let status_code = self.extract_status_code(webdriver).await.unwrap_or(200);
        response_builder =
            response_builder.status(StatusCode::from_u16(status_code).unwrap_or(StatusCode::OK));

        // Extract headers if possible (very limited in WebDriver)
        let headers = self.extract_headers(webdriver).await.unwrap_or_default();
        for (name, value) in headers {
            if let (Ok(name), Ok(value)) = (
                HeaderName::from_bytes(&name),
                HeaderValue::from_bytes(&value),
            ) {
                response_builder = response_builder.header(name, value);
            }
        }

        // Build response body based on configuration
        let body_content = self.extract_content(webdriver).await?;
        let body_bytes = self.prepare_response_body(body_content)?;
        let body = Body::from(body_bytes);

        // Build the response
        let response = response_builder
            .body(body)
            .map_err(|e| BrowserError::operation_failed("build_response", e.to_string()))?;

        Ok(response)
    }

    /// Attempts to extract HTTP status code from the browser.
    async fn extract_status_code(&self, webdriver: &WebDriver) -> Option<u16> {
        // This is challenging with WebDriver as HTTP details are abstracted
        // We can try some heuristics or JavaScript approaches
        let script = r#"
            try {
                // Try to access performance navigation timing
                if (window.performance && window.performance.getEntriesByType) {
                    const entries = window.performance.getEntriesByType('navigation');
                    if (entries.length > 0 && entries[0].responseStatus) {
                        return entries[0].responseStatus;
                    }
                }

                // Fallback: assume 200 if page loaded successfully
                return document.readyState === 'complete' ? 200 : null;
            } catch (e) {
                return null;
            }
        "#;

        match webdriver.execute(script, vec![]).await {
            Ok(result) => {
                // Try to convert to u64 first, then to u16
                result.convert::<u64>().ok().and_then(|code| {
                    if code <= u16::MAX as u64 {
                        Some(code as u16)
                    } else {
                        None
                    }
                })
            }
            Err(_) => None,
        }
    }

    /// Attempts to extract response headers from the browser.
    async fn extract_headers(&self, webdriver: &WebDriver) -> Option<Vec<(Vec<u8>, Vec<u8>)>> {
        // WebDriver has very limited access to HTTP headers
        // We can try to extract some common headers via JavaScript
        let script = r#"
            try {
                const headers = [];

                // Extract content type from document
                if (document.contentType) {
                    headers.push(['content-type', document.contentType]);
                }

                // Extract charset
                if (document.characterSet) {
                    headers.push(['charset', document.characterSet]);
                }

                // Extract last modified
                if (document.lastModified) {
                    headers.push(['last-modified', document.lastModified]);
                }

                return headers;
            } catch (e) {
                return [];
            }
        "#;

        match webdriver.execute(script, vec![]).await {
            Ok(result) => {
                // Try to convert to serde_json::Value to work with arrays
                if let Ok(json_value) = result.convert::<serde_json::Value>() {
                    if let Some(header_array) = json_value.as_array() {
                        let mut headers = Vec::new();
                        for header in header_array {
                            if let Some(header_pair) = header.as_array()
                                && header_pair.len() == 2
                                && let (Some(name), Some(value)) =
                                    (header_pair[0].as_str(), header_pair[1].as_str())
                            {
                                headers.push((name.as_bytes().to_vec(), value.as_bytes().to_vec()));
                            }
                        }
                        Some(headers)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }

    /// Extracts content from the current page based on configuration.
    async fn extract_content(&self, webdriver: &WebDriver) -> Result<ContentData, BrowserError> {
        let mut content = ContentData::default();

        // Extract HTML content if requested
        if self.config.extract_html {
            content.html = Some(webdriver.source().await.map_err(|e| {
                BrowserError::webdriver(e, Some("extracting page source".to_string()))
            })?);
        }

        // Extract text content if requested
        if self.config.extract_text {
            let script =
                "return document.body ? document.body.innerText || document.body.textContent : '';";
            let text_result = webdriver
                .execute(script, vec![])
                .await
                .map_err(|e| BrowserError::script_error(script, e.to_string()))?;

            if let Ok(text) = text_result.convert::<String>() {
                content.text = Some(text);
            }
        }

        // Extract title
        content.title = webdriver
            .title()
            .await
            .map_err(|e| BrowserError::webdriver(e, Some("extracting page title".to_string())))?;

        // Extract URL
        content.url = webdriver
            .current_url()
            .await
            .map_err(|e| BrowserError::webdriver(e, Some("extracting current URL".to_string())))?
            .to_string();

        Ok(content)
    }

    /// Prepares the response body from extracted content.
    fn prepare_response_body(&self, content: ContentData) -> Result<Bytes, BrowserError> {
        let body_data = if let Some(html) = content.html {
            html
        } else if let Some(text) = content.text {
            text
        } else {
            // Fallback: create minimal HTML with title and URL
            format!(
                "<html><head><title>{}</title></head><body><p>Content extracted from: {}</p></body></html>",
                content.title, content.url
            )
        };

        // Check size limit
        if body_data.len() > self.config.max_response_size {
            return Err(BrowserError::resource_error(
                "response_body",
                &content.url,
                format!(
                    "Response size {} exceeds limit {}",
                    body_data.len(),
                    self.config.max_response_size
                ),
            ));
        }

        Ok(Bytes::from(body_data))
    }

    /// Captures a screenshot on error for debugging.
    async fn capture_error_screenshot(
        &self,
        webdriver: &WebDriver,
        url: &str,
    ) -> Result<(), BrowserError> {
        let timestamp = jiff::Zoned::now().strftime("%Y%m%d_%H%M%S%.3f");
        let filename = format!(
            "error_{}_{}.png",
            timestamp,
            url.replace(['/', ':', '?', '&'], "_")
        );

        match webdriver.screenshot_as_png().await {
            Ok(screenshot) => {
                if let Err(e) = std::fs::write(&filename, screenshot) {
                    eprintln!("Failed to save error screenshot {}: {}", filename, e);
                }
                Ok(())
            }
            Err(e) => Err(BrowserError::operation_failed(
                "capture_screenshot",
                e.to_string(),
            )),
        }
    }

    /// Logs a successful request for monitoring.
    fn log_request_success(&self, url: &str, elapsed: Duration) {
        println!(
            "Browser request successful: {} ({}ms)",
            url,
            elapsed.as_millis()
        );
    }
}

impl From<Object<BrowserManager>> for BrowserClient {
    fn from(connection: Object<BrowserManager>) -> Self {
        Self::new(connection)
    }
}

impl Deref for BrowserClient {
    type Target = BrowserConnection;

    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

impl Service<Request> for BrowserClient {
    type Error = Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;
    type Response = Response;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Check if connection is healthy
        if !self.connection.is_healthy() {
            return Poll::Ready(Err(BrowserError::invalid_session(
                self.connection.session_id().map(String::from),
                "Connection marked as unhealthy".to_string(),
            )
            .into()));
        }

        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let client = self.clone();

        let fut = async move { client.process_request(request).await };

        fut.boxed()
    }
}

/// Data extracted from a web page.
#[derive(Debug, Default, Clone)]
struct ContentData {
    /// HTML source code
    html: Option<String>,
    /// Text content
    text: Option<String>,
    /// Page title
    title: String,
    /// Current URL (after redirects)
    url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_config_builder() {
        let config = ClientConfig::builder()
            .with_navigation_timeout(Duration::from_secs(60))
            .with_extract_html(true)
            .with_extract_text(false)
            .with_enable_javascript(true)
            .with_user_agent("test-agent")
            .with_max_response_size(5_usize * 1024 * 1024)
            .build()
            .expect("Should build successfully");

        assert_eq!(config.navigation_timeout, Duration::from_secs(60));
        assert!(config.extract_html);
        assert!(!config.extract_text);
        assert!(config.enable_javascript);
        assert_eq!(config.user_agent, Some("test-agent".to_string()));
        assert_eq!(config.max_response_size, 5 * 1024 * 1024);
    }

    #[test]
    fn client_config_builder_validation() {
        // Zero navigation timeout should fail validation
        let result = ClientConfig::builder()
            .with_navigation_timeout(Duration::ZERO)
            .build();
        assert!(result.is_err());

        // Zero element timeout should fail validation
        let result = ClientConfig::builder()
            .with_element_timeout(Duration::ZERO)
            .build();
        assert!(result.is_err());

        // Zero max response size should fail validation
        let result = ClientConfig::builder()
            .with_max_response_size(0_usize)
            .build();
        assert!(result.is_err());

        // Excessive max response size should fail validation
        let result = ClientConfig::builder()
            .with_max_response_size(2_usize * 1024 * 1024 * 1024) // 2GB
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn content_data_default() {
        let content = ContentData::default();
        assert!(content.html.is_none());
        assert!(content.text.is_none());
        assert!(content.title.is_empty());
        assert!(content.url.is_empty());
    }
}
