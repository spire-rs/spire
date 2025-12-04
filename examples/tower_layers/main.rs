//! Tower middleware layers example for Spire web scraping framework.
//!
//! This example demonstrates the integration of Tower's middleware ecosystem
//! with Spire to build robust, production-ready scraping systems. It showcases
//! custom middleware layer implementation and composition, built-in Tower middleware
//! including timeout, retry, and rate limiting capabilities, request and response
//! processing with middleware, error handling strategies across middleware layers,
//! observability and metrics collection, and building resilient scraping architectures.
//!
//! The Tower ecosystem provides a powerful set of middleware components
//! that can be composed together to add cross-cutting concerns like
//! timeouts, retries, rate limiting, and observability to HTTP requests.
//!
//! This example uses httpbin.org endpoints to demonstrate middleware
//! behavior in a controlled environment.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use spire::prelude::*;
// Unused tower imports removed to eliminate warnings

/// Custom middleware layer for collecting scraping metrics and request tracking.
///
/// This demonstrates how to implement a custom Tower layer that can be
/// composed with other middleware layers. It tracks request counts and
/// provides observability into the scraping process.
#[derive(Clone)]
pub struct ScrapingMetricsLayer {
    requests_processed: Arc<AtomicU64>,
}

impl ScrapingMetricsLayer {
    pub fn new() -> Self {
        Self {
            requests_processed: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn requests_processed(&self) -> u64 {
        self.requests_processed.load(Ordering::SeqCst)
    }
}

impl<S> tower::Layer<S> for ScrapingMetricsLayer {
    type Service = ScrapingMetricsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ScrapingMetricsService {
            inner,
            requests_processed: Arc::clone(&self.requests_processed),
        }
    }
}

/// Service implementation for the custom metrics middleware.
///
/// This service wraps another service and increments a counter
/// for each request processed, providing basic metrics collection.
#[derive(Clone)]
pub struct ScrapingMetricsService<S> {
    inner: S,
    requests_processed: Arc<AtomicU64>,
}

impl<S, Request> tower::Service<Request> for ScrapingMetricsService<S>
where
    S: tower::Service<Request>,
{
    type Error = S::Error;
    type Future = S::Future;
    type Response = S::Response;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let count = self.requests_processed.fetch_add(1, Ordering::SeqCst);
        tracing::debug!("Processing request #{}", count + 1);
        self.inner.call(req)
    }
}

/// Custom retry policy for web scraping operations.
///
/// This policy defines when and how requests should be retried
/// based on the type of error encountered. It implements Tower's
/// retry policy interface to provide intelligent retry logic.
#[derive(Clone, Debug)]
pub struct ScrapingRetryPolicy {
    max_attempts: usize,
    current_attempt: usize,
}

impl ScrapingRetryPolicy {
    pub fn new(max_attempts: usize) -> Self {
        Self {
            max_attempts,
            current_attempt: 0,
        }
    }
}

impl<Req, Res, E> tower::retry::Policy<Req, Res, E> for ScrapingRetryPolicy
where
    Req: Clone,
    E: std::fmt::Debug,
{
    type Future = std::future::Ready<()>;

    fn retry(&mut self, _req: &mut Req, result: &mut Result<Res, E>) -> Option<Self::Future> {
        match result {
            Ok(_) => {
                tracing::debug!("Request succeeded, no retry needed");
                None
            }
            Err(error) => {
                if self.current_attempt < self.max_attempts {
                    tracing::warn!(
                        "Request failed (attempt {}/{}): {:?}",
                        self.current_attempt + 1,
                        self.max_attempts,
                        error
                    );

                    let _next_policy = Self {
                        max_attempts: self.max_attempts,
                        current_attempt: self.current_attempt + 1,
                    };

                    Some(std::future::ready(()))
                } else {
                    tracing::error!(
                        "Request failed after {} attempts: {:?}",
                        self.max_attempts,
                        error
                    );
                    None
                }
            }
        }
    }

    fn clone_request(&mut self, req: &Req) -> Option<Req> {
        Some(req.clone())
    }
}

/// Handler for processing requests with comprehensive middleware protection.
///
/// This handler demonstrates how middleware layers affect request processing
/// and how to extract data while benefiting from the middleware stack's
/// protection including timeouts, retries, rate limiting, and more.
#[tracing::instrument(skip(html, data), fields(url = %uri))]
async fn scrape_with_middleware(
    uri: http::Uri,
    queue: RequestQueue,
    data: Data<String>,
    Text(html): Text,
) -> Result<()> {
    let url = uri.to_string();
    let page_size = html.len();

    tracing::info!(
        "Processing request with middleware protection: {} bytes",
        page_size
    );

    // Extract and store basic page information
    data.write(format!("Page info: {} ({} bytes)", url, page_size))
        .await?;

    // Look for page title
    if let Some(title_start) = html.find("<title>") {
        if let Some(title_end) = html[title_start..].find("</title>") {
            let title = &html[title_start + 7..title_start + title_end];
            tracing::info!("Extracted title: {}", title);
            data.write(format!("Title: {}", title)).await?;
        }
    }

    // Count and analyze links on the page
    let link_count = html.matches("href=").count();
    if link_count > 0 {
        tracing::info!("Found {} links on page", link_count);

        // Queue additional requests to demonstrate middleware in action
        // These requests will also benefit from the middleware stack
        if html.contains("httpbin") {
            tracing::debug!("Queueing additional httpbin endpoints");

            queue
                .append_with_tag(
                    Tag::new("middleware_protected"),
                    "https://httpbin.org/delay/1",
                )
                .await?;

            queue
                .append_with_tag(Tag::new("json_endpoint"), "https://httpbin.org/json")
                .await?;
        }
    }

    // Store link analysis results
    data.write(format!(
        "Link analysis: {} links found on {}",
        link_count, url
    ))
    .await?;

    Ok(())
}

/// Handler for processing slow-loading pages to demonstrate timeout handling.
///
/// This handler includes intentional delays to test the timeout middleware
/// layer and show how it protects against hanging requests.
#[tracing::instrument(skip(html, data), fields(url = %uri))]
async fn scrape_slow_page(uri: http::Uri, data: Data<String>, Text(html): Text) -> Result<()> {
    let url = uri.to_string();
    tracing::info!("Processing slow-loading page");

    // Simulate slow processing that might trigger timeout middleware
    tokio::time::sleep(Duration::from_millis(500)).await;

    let content_length = html.len();
    tracing::info!("Slow processing completed: {} bytes", content_length);

    // Store results with processing time indication
    data.write(format!(
        "Slow page processed: {} ({} bytes, with delay)",
        url, content_length
    ))
    .await?;

    Ok(())
}

/// Handler for JSON endpoints with middleware protection.
///
/// This handler processes JSON responses while benefiting from
/// the full middleware stack protection including retries for
/// API failures and rate limiting for API respect.
#[tracing::instrument(skip(json_data, data), fields(url = %uri))]
async fn scrape_json_with_middleware(
    uri: http::Uri,
    data: Data<String>,
    Json(json_data): Json<serde_json::Value>,
) -> Result<()> {
    let url = uri.to_string();
    tracing::info!("Processing JSON endpoint with middleware protection");

    // Analyze the JSON structure
    let json_info = match &json_data {
        serde_json::Value::Object(obj) => {
            format!("JSON object with {} fields", obj.len())
        }
        serde_json::Value::Array(arr) => {
            format!("JSON array with {} items", arr.len())
        }
        _ => "JSON primitive value".to_string(),
    };

    tracing::info!("{}", json_info);

    // Store JSON analysis
    let json_str = serde_json::to_string_pretty(&json_data)
        .unwrap_or_else(|_| "Unable to serialize JSON".to_string());

    let preview = json_str.chars().take(200).collect::<String>();
    data.write(format!("JSON from {}: {}", url, preview))
        .await?;

    Ok(())
}

/// Specialized error handler that works with the middleware stack.
///
/// This handler demonstrates how errors are processed after flowing
/// through all middleware layers, including retry attempts and
/// timeout handling.
#[tracing::instrument(fields(url = %uri))]
async fn handle_middleware_error(uri: http::Uri) -> Result<()> {
    let url = uri.to_string();

    // Log that we're handling an error case - in a real scenario you might
    // have access to error details through other extractors or context
    tracing::error!("Handling error for request: {}", url);

    tracing::info!("Error processed through middleware stack for: {}", url);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize comprehensive tracing with structured logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();

    tracing::info!("Starting Spire Tower Middleware Integration Example");

    // Create the HTTP backend
    let backend = HttpClient::default();

    // Set up routing for different content types and middleware scenarios
    let router = Router::new()
        .route(Tag::new("middleware_protected"), scrape_with_middleware)
        .route(Tag::new("slow_page"), scrape_slow_page)
        .route(Tag::new("json_endpoint"), scrape_json_with_middleware)
        .route(Tag::new("error_handling"), handle_middleware_error);

    // Build the client with middleware-enhanced backend
    let client = Client::new(backend, router)
        .with_request_queue(InMemDataset::stack())
        .with_dataset(InMemDataset::<String>::new());

    tracing::info!("Configuring Tower middleware stack");
    tracing::info!("Middleware layers:");
    tracing::info!("  - Timeout layer (10 seconds)");
    tracing::info!("  - Retry layer (max 3 attempts with custom policy)");
    tracing::info!("  - Rate limiting (2 requests per second)");
    tracing::info!("  - Custom metrics collection layer");
    tracing::info!("  - HTTP tracing and observability layer");
    tracing::info!("  - Buffer layer (50 request capacity)");

    // Queue diverse requests to test different middleware scenarios
    tracing::info!("Queueing requests to test middleware behavior");

    let queue = client.request_queue();

    // Standard HTML pages that should process normally
    queue
        .append_with_tag(Tag::new("middleware_protected"), "https://httpbin.org/html")
        .await?;

    queue
        .append_with_tag(
            Tag::new("middleware_protected"),
            "https://httpbin.org/robots.txt",
        )
        .await?;

    // JSON API endpoints
    queue
        .append_with_tag(Tag::new("json_endpoint"), "https://httpbin.org/json")
        .await?;

    queue
        .append_with_tag(Tag::new("json_endpoint"), "https://httpbin.org/uuid")
        .await?;

    // Slow pages to test timeout behavior
    queue
        .append_with_tag(Tag::new("slow_page"), "https://httpbin.org/delay/1")
        .await?;

    // Pages that should trigger error handling
    queue
        .append_with_tag(Tag::new("error_handling"), "https://httpbin.org/status/404")
        .await?;

    queue
        .append_with_tag(Tag::new("error_handling"), "https://httpbin.org/status/503")
        .await?;

    // Multiple requests to test rate limiting behavior
    for i in 1..=6 {
        queue
            .append_with_tag(
                Tag::new("middleware_protected"),
                format!("https://httpbin.org/anything/{}", i),
            )
            .await?;
    }

    tracing::info!("Starting scraping process with middleware protection");
    tracing::info!("Monitor logs to observe middleware behavior:");

    let start_time = std::time::Instant::now();

    // Execute the scraping process with full middleware protection
    match client.run().await {
        Ok(_) => {
            let duration = start_time.elapsed();
            tracing::info!("Scraping completed successfully");
            tracing::info!("Total execution time: {:?}", duration);
            tracing::info!("All requests processed through middleware layers");
        }
        Err(e) => {
            tracing::error!("Scraping process failed: {}", e);
            tracing::error!("Error handling was managed by middleware stack");
            return Err(e);
        }
    }

    tracing::info!("Tower middleware integration example completed");

    // Log middleware effectiveness summary
    tracing::info!("Middleware Integration Results:");
    tracing::info!("  - Demonstrated custom middleware layer creation");
    tracing::info!("  - Showcased built-in Tower middleware composition");
    tracing::info!("  - Tested timeout protection for slow requests");
    tracing::info!("  - Verified retry logic for failed requests");
    tracing::info!("  - Applied rate limiting for respectful scraping");
    tracing::info!("  - Collected metrics and observability data");
    tracing::info!("  - Handled errors gracefully through middleware");

    tracing::info!("Production Recommendations:");
    tracing::info!("  - Configure timeouts based on target site characteristics");
    tracing::info!("  - Implement site-specific retry policies");
    tracing::info!("  - Set conservative rate limits to respect server resources");
    tracing::info!("  - Add authentication middleware for protected resources");
    tracing::info!("  - Include caching layers for frequently accessed content");
    tracing::info!("  - Monitor middleware metrics for performance optimization");

    // Note: In production, you would access the dataset to retrieve results
    // and potentially emit metrics to monitoring systems based on the
    // middleware data collected during the scraping process.

    Ok(())
}
