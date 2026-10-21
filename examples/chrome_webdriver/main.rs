//! Chrome `WebDriver` automation example demonstrating browser-based scraping with Spire.
//!
//! This example showcases the browser automation capabilities of the Spire framework
//! using the thirtyfour backend with Chrome `WebDriver`. It demonstrates how to set up
//! browser-based web scraping for JavaScript-heavy pages, handle dynamic content
//! loading, extract data from rendered web pages, and manage browser automation
//! workflows with proper error handling.
//!
//! The example covers basic browser page processing, dynamic content handling,
//! `WebDriver` integration patterns, and graceful error handling when `WebDriver`
//! services are unavailable.
//!
//! Prerequisites: Chrome browser must be installed and accessible.
//! `ChromeDriver` is automatically managed by the thirtyfour library.

mod data;

use std::time::Duration;

use data::PageData;
use spire::dataset::{Data, InMemDataset};
use spire::extract::Html;
use spire::extract::driver::View;
use spire::frontier::Seed;
use spire::{BrowserBackend, Crawler, Result, Router, http};

/// Handler for processing pages that require browser rendering.
///
/// This handler demonstrates basic browser navigation and content extraction
/// using `WebDriver` for JavaScript-heavy or dynamically rendered pages. It shows
/// how to wait for page content to load, extract basic page information, and
/// store structured data about browser-rendered pages.
async fn scrape_browser_page(
    uri: http::Uri,
    data_store: Data<PageData>,
    view: View,
    page: Html,
) -> Result<()> {
    let url = uri.to_string();
    tracing::info!("Processing browser page: {}", url);

    // Allow time for page content to load and JavaScript to execute
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Extract page information using WebDriver
    let title = view
        .title()
        .await
        .map_err(|e| spire::Error::backend(format!("Failed to get title: {e}")))?;

    let current_url = view
        .current_url()
        .await
        .map_err(|e| spire::Error::backend(format!("Failed to get current URL: {e}")))?;

    tracing::info!("Extracted title: {} from {}", title, current_url);

    let page_data = PageData::new(
        current_url.to_string(),
        title,
        page.html().len(),
        "Browser".to_string(),
    );

    data_store.write(page_data).await?;
    Ok(())
}

/// Handler for processing pages with dynamic content loading.
///
/// This handler demonstrates handling JavaScript-heavy pages that require
/// additional time for dynamic content to load and render. It shows extended
/// wait patterns for content that loads asynchronously via JavaScript.
async fn scrape_dynamic_content(
    uri: http::Uri,
    data_store: Data<PageData>,
    view: View,
    page: Html,
) -> Result<()> {
    let url = uri.to_string();
    tracing::info!("Processing dynamic content page: {}", url);

    // Wait longer for dynamic content to load
    tokio::time::sleep(Duration::from_secs(3)).await;

    let title = view
        .title()
        .await
        .map_err(|e| spire::Error::backend(format!("Failed to get title: {e}")))?;

    let current_url = view
        .current_url()
        .await
        .map_err(|e| spire::Error::backend(format!("Failed to get current URL: {e}")))?;

    tracing::info!("Dynamic content loaded: {} from {}", title, current_url);

    let page_data = PageData::new(
        current_url.to_string(),
        title,
        page.html().len(),
        "Dynamic Browser".to_string(),
    );

    data_store.write(page_data).await?;
    Ok(())
}

/// Creates the crawler with its `WebDriver` backend and initial requests.
fn create_crawler() -> Result<Crawler<BrowserBackend>> {
    let browser_backend = BrowserBackend::builder()
        .remote("http://127.0.0.1:4444")
        .build()?;

    let router = Router::new()
        .route("browser_page", scrape_browser_page)
        .route("dynamic_content", scrape_dynamic_content);

    let crawler = Crawler::new(browser_backend, router)
        .with_dataset(InMemDataset::<PageData>::new())
        .with_seed(Seed::get("https://httpbin.org/html")?.with_tag("browser_page"))
        .with_seed(Seed::get("https://httpbin.org/delay/1")?.with_tag("dynamic_content"));

    Ok(crawler)
}

/// Displays the browser automation results from processed pages.
async fn display_results(dataset: &Data<PageData>) -> Result<()> {
    let results = dataset.read_all().await?;

    tracing::info!(
        "Browser automation completed! Processed {} pages:",
        results.len()
    );

    for (i, page_data) in results.iter().enumerate() {
        tracing::info!(
            "{}: {} - {} ({} bytes)",
            i + 1,
            page_data.title,
            page_data.url,
            page_data.content_length
        );
        tracing::info!("   Processing method: {}", page_data.processing_method);
        tracing::info!("   Processed at: {}", page_data.processed_at);
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize structured logging
    tracing_subscriber::fmt().init();

    tracing::info!("Starting Spire Chrome WebDriver Example");

    // Note: This example requires Chrome browser and WebDriver setup
    tracing::info!("Ensure Chrome browser is installed and accessible");
    tracing::info!("WebDriver will attempt to connect to http://127.0.0.1:4444");

    // Attempt to create the crawler
    match create_crawler() {
        Ok(crawler) => {
            tracing::info!("Browser backend initialized successfully");

            tracing::info!("Starting browser automation");
            let stats = crawler.run().await?;
            tracing::info!(?stats, "Browser automation finished");

            // Display results
            let dataset = crawler.dataset::<PageData>()?;
            display_results(&dataset).await?;

            // Quit the browser sessions instead of dropping them.
            crawler.backend().close().await;

            tracing::info!("Chrome WebDriver example completed successfully");
        }
        Err(e) => {
            tracing::error!("Failed to initialize browser backend: {}", e);
            tracing::warn!("This may indicate WebDriver setup issues:");
            tracing::info!("  - Chrome browser not installed or not in PATH");
            tracing::info!("  - ChromeDriver compatibility issues");
            tracing::info!("  - Network connectivity problems");
            tracing::info!("  - Insufficient system resources");
            tracing::info!("  - WebDriver server not running on port 4444");
            tracing::info!("Example structure demonstrated, but browser automation skipped");
        }
    }

    Ok(())
}
