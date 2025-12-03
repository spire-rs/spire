//! Basic usage example demonstrating core Spire functionality.
//!
//! This example demonstrates fundamental concepts of the Spire framework:
//! - Setting up an HTTP scraping client with the reqwest backend
//! - Defining handlers for different content types using extractors
//! - Managing request queues with tag-based routing
//! - Extracting and processing text and JSON data from responses
//! - Storing scraped data using in-memory datasets
//! - Error handling using Spire's Result types
//!
//! The example scrapes test endpoints from httpbin.org to demonstrate
//! real HTTP interactions without relying on external services that
//! might change or become unavailable.

use std::collections::HashMap;

use spire::prelude::*;
use tracing::{error, info};

/// Handler for scraping HTML pages and extracting basic information.
///
/// This handler demonstrates:
/// - Text extraction from HTML responses
/// - Simple HTML parsing for title extraction
/// - Queueing additional requests based on page content
/// - Storing extracted data in the dataset
async fn scrape_html_page(Text(html): Text) -> Result<()> {
    info!("Processing HTML page: {} bytes", html.len());
    Ok(())
}

/// Handler for processing JSON API responses.
///
/// This handler demonstrates:
/// - JSON deserialization from HTTP responses
/// - Processing structured data from APIs
/// - Handling dynamic JSON structures
async fn scrape_json_api(
    uri: http::Uri,
    data_store: Data<String>,
    Json(data): Json<HashMap<String, serde_json::Value>>,
) -> Result<()> {
    let url = uri.to_string();
    info!("Processing JSON API: {}", url);

    let field_count = data.len();
    info!("JSON response contains {} fields", field_count);

    // Process each field in the JSON response
    for (key, value) in data {
        let entry = format!("API field '{}': {}", key, value);
        info!("Extracted: {}", entry);
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
/// This handler demonstrates:
/// - Generic text processing for various content types
/// - Content-type aware processing
/// - Basic text analysis and metrics
async fn scrape_text_content(
    uri: http::Uri,
    data: Data<String>,
    Text(content): Text,
) -> Result<()> {
    let url = uri.to_string();
    info!("Processing text content: {}", url);

    let content_length = content.len();
    let line_count = content.lines().count();
    let word_count = content.split_whitespace().count();

    info!(
        "Text analysis: {} bytes, {} lines, {} words",
        content_length, line_count, word_count
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

// /// Handler for failed requests.
// ///
// /// This handler demonstrates:
// /// - Graceful error handling for different HTTP status codes
// /// - Logging error information for debugging
// /// - Converting errors to Spire Result types
// async fn handle_error(uri: Uri, response: Response) -> Result<()> {
//     let url = uri.to_string();

//     let status = response.status();
//     let status_code = status.as_u16();

//     match status_code {
//         404 => {
//             warn!("Resource not found: {}", url);
//         }
//         403 => {
//             warn!(
//                 "Access forbidden: {} - check permissions or rate limiting",
//                 url
//             );
//         }
//         429 => {
//             warn!("Rate limited: {} - consider implementing delays", url);
//         }
//         500..=599 => {
//             error!("Server error {} for: {}", status_code, url);
//         }
//         _ => {
//             error!("HTTP error {} for: {}", status_code, url);
//         }
//     }

//     Ok(())
// }

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

    info!("Starting Spire Basic Usage Example");

    // Create a router that maps tags to handler functions
    // Each tag represents a different type of content to be processed
    let router = Router::new()
        .route(Tag::new("html_page"), scrape_html_page)
        .route(Tag::new("json_api"), scrape_json_api)
        .route(Tag::new("text_content"), scrape_text_content);
    // .route(Tag::new("error"), handle_error);

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
    info!("Queueing initial requests");

    let queue = client.request_queue();

    // HTML page with potential links to other content
    queue
        .push(Tag::new("html_page"), "https://httpbin.org/html")
        .await?;

    // JSON API endpoint
    queue
        .push(Tag::new("json_api"), "https://httpbin.org/json")
        .await?;

    // Plain text content
    queue
        .push(Tag::new("text_content"), "https://httpbin.org/robots.txt")
        .await?;

    // Intentional error to demonstrate error handling
    // queue
    //     .push(Tag::new("error"), "https://httpbin.org/status/404")
    //     .await?;

    info!("Starting scraping process");

    // Execute the scraping process
    // This will process all queued requests and any additional requests
    // that are queued by handlers during processing
    match client.run().await {
        Ok(_) => {
            info!("Scraping completed successfully");
        }
        Err(e) => {
            error!("Scraping process failed: {}", e);
            return Err(e);
        }
    }

    info!("Basic usage example completed");

    // Note: In a real application, you would typically access the dataset
    // to retrieve and process the stored results:
    //
    // let results = client.dataset().get_all().await?;
    // for result in results {
    //     println!("Scraped: {}", result);
    // }

    Ok(())
}
