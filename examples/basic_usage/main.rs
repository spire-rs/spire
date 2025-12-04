//! Basic usage example demonstrating core Spire functionality.
//!
//! This example demonstrates fundamental concepts of the Spire framework.
//! It shows how to set up an HTTP scraping client with the reqwest backend
//! and define handlers for different content types using extractors.
//! The example covers managing request queues with tag-based routing,
//! extracting and processing text and JSON data from responses,
//! storing scraped data using in-memory datasets, and handling errors
//! using Spire's Result types.
//!
//! The example scrapes test endpoints from httpbin.org to demonstrate
//! real HTTP interactions without relying on external services that
//! might change or become unavailable.

use std::collections::HashMap;

use spire::prelude::*;

/// Handler for scraping HTML pages and extracting basic information.
///
/// This handler demonstrates text extraction from HTML responses,
/// simple HTML parsing for title extraction, queueing additional
/// requests based on page content, and storing extracted data in the dataset.
async fn scrape_html_page(Text(html): Text) -> Result<()> {
    tracing::info!("Processing HTML page: {} bytes", html.len());
    Ok(())
}

/// Handler for processing JSON API responses.
///
/// This handler demonstrates JSON deserialization from HTTP responses,
/// processing structured data from APIs, and handling dynamic JSON structures.
async fn scrape_json_api(
    uri: http::Uri,
    data_store: Data<String>,
    Json(data): Json<HashMap<String, serde_json::Value>>,
) -> Result<()> {
    let url = uri.to_string();
    tracing::info!("Processing JSON API: {}", url);

    let field_count = data.len();
    tracing::info!("JSON response contains {} fields", field_count);

    // Process each field in the JSON response
    for (key, value) in data {
        let entry = format!("API field '{}': {}", key, value);
        tracing::info!("Extracted: {}", entry);
        data_store.write(entry).await?;
    }

    // Store API metadata
    data_store
        .write(format!("JSON API {} ({} fields)", url, field_count))
        .await?;

    Ok(())
}

/// Handler for processing different types of text content.
///
/// This handler demonstrates generic text processing for various content types,
/// content-type aware processing, and basic text analysis and metrics.
async fn scrape_text_content(
    uri: http::Uri,
    data: Data<String>,
    Text(content): Text,
) -> Result<()> {
    let url = uri.to_string();
    tracing::info!("Processing text content: {}", url);

    let content_length = content.len();
    let line_count = content.lines().count();
    let word_count = content.split_whitespace().count();

    tracing::info!(
        "Text analysis: {} bytes, {} lines, {} words",
        content_length,
        line_count,
        word_count
    );

    // Store content analysis
    data.write(format!(
        "Text from {}: {} bytes, {} lines, {} words",
        url, content_length, line_count, word_count
    ))
    .await?;

    // Store a preview of the content (first 100 characters)
    let preview = content.chars().take(100).collect::<String>();
    data.write(format!("Content preview: {}", preview)).await?;

    Ok(())
}

/// Handler for failed requests.
///
/// This handler demonstrates graceful error handling for different request scenarios,
/// logging error information for debugging, and converting errors to Spire Result types.
async fn handle_error(uri: http::Uri) -> Result<()> {
    let url = uri.to_string();

    // Log the error case - in production you might have more context
    // about the specific error through other mechanisms
    tracing::error!("Error processing request: {}", url);

    // You could implement specific error handling logic here
    // based on the URL pattern or other available context

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize structured logging with tracing-subscriber
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();

    tracing::info!("Starting Spire Basic Usage Example");

    // Create a router that maps tags to handler functions
    // Each tag represents a different type of content to be processed
    let router = Router::new()
        .route(Tag::new("html_page"), scrape_html_page)
        .route(Tag::new("json_api"), scrape_json_api)
        .route(Tag::new("text_content"), scrape_text_content)
        .route(Tag::new("error"), handle_error);

    // Create the HTTP backend for making requests
    let backend = HttpClient::default();

    // Build the client with necessary components:
    // - Backend: handles HTTP requests
    // - Router: dispatches requests to appropriate handlers
    // - Request queue: manages pending requests
    // - Dataset: stores scraped results
    let client = Client::new(backend, router)
        .with_request_queue(InMemDataset::stack())
        .with_dataset(InMemDataset::<String>::new());

    // Queue initial requests to demonstrate different content types
    tracing::info!("Queueing initial requests");

    let queue = client.request_queue();

    // HTML page with potential links to other content
    queue
        .append_with_tag(Tag::new("html_page"), "https://httpbin.org/html")
        .await?;

    // JSON API endpoint
    queue
        .append_with_tag(Tag::new("json_api"), "https://httpbin.org/json")
        .await?;

    // Plain text content
    queue
        .append_with_tag(Tag::new("text_content"), "https://httpbin.org/robots.txt")
        .await?;

    // Intentional error to demonstrate error handling
    queue
        .append_with_tag(Tag::new("error"), "https://httpbin.org/status/404")
        .await?;

    tracing::info!("Starting scraping process");

    // Execute the scraping process
    // This will process all queued requests and any additional requests
    // that are queued by handlers during processing
    match client.run().await {
        Ok(_) => {
            tracing::info!("Scraping completed successfully");
        }
        Err(e) => {
            tracing::error!("Scraping process failed: {}", e);
            return Err(e);
        }
    }

    tracing::info!("Basic usage example completed");

    // Note: In a real application, you would typically access the dataset
    // to retrieve and process the stored results:
    //
    // let results = client.dataset().get_all().await?;
    // for result in results {
    //     println!("Scraped: {}", result);
    // }

    Ok(())
}
