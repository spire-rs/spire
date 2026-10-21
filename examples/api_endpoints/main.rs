//! API endpoints example demonstrating JSON API processing with Spire.
//!
//! This example shows fundamental JSON API processing capabilities of the Spire
//! framework. It demonstrates how to handle different JSON response structures,
//! work with typed and dynamic JSON data, manage multiple datasets for different
//! data types, and process API responses with proper error handling and validation.
//!
//! The example covers processing structured JSON APIs, dynamic JSON handling,
//! text content analysis, and endpoint discovery from API responses.

mod data;

use std::collections::HashMap;

use data::{ApiMetadata, User};
use serde_json::Value;
use spire::dataset::{Data, InMemDataset};
use spire::extract::{Json, Text};
use spire::frontier::{FrontierExt, InMemFrontier, Queue, Seed};
use spire::{Crawler, HttpClient, Result, Router, http};

/// Handler for processing structured JSON API responses.
///
/// This handler demonstrates JSON extraction and processing of structured
/// API responses, converting JSON data to User domain objects and storing
/// them in the user dataset.
async fn process_structured_json(
    uri: http::Uri,
    data_store: Data<User>,
    Json(json_data): Json<Value>,
) -> Result<()> {
    let url = uri.to_string();
    tracing::info!("Processing structured JSON from: {}", url);

    // Create a user from JSON data
    let user = User::from_json(&json_data, url.clone());

    tracing::info!(
        "Processed user: {} (ID: {}, URL: {})",
        user.name,
        user.id,
        user.source_url
    );

    data_store.write(user).await?;
    Ok(())
}

/// Handler for processing dynamic JSON API responses.
///
/// This handler demonstrates working with unstructured JSON data, extracting
/// metadata from API responses, and analyzing JSON structure dynamically.
async fn process_dynamic_json(
    uri: http::Uri,
    data_store: Data<ApiMetadata>,
    Json(data): Json<Value>,
) -> Result<()> {
    let url = uri.to_string();
    tracing::info!("Processing dynamic JSON from: {}", url);

    let json_string = serde_json::to_string(&data)
        .map_err(|e| spire::Error::worker("failed to serialize JSON").with_source(e))?;

    let fields_count = count_json_fields(&data);

    tracing::info!(
        "JSON response: {} fields, {} bytes",
        fields_count,
        json_string.len()
    );

    let metadata = ApiMetadata::new(url, json_string.len(), fields_count);
    data_store.write(metadata).await?;

    // Log some interesting fields if they exist
    if let Some(obj) = data.as_object() {
        for (key, value) in obj.iter().take(5) {
            tracing::info!("Field '{}': {}", key, format_json_value(value));
        }
    }

    Ok(())
}

/// Handler for processing text content from API endpoints.
///
/// This handler demonstrates text processing and basic content analysis
/// for API responses that return plain text or mixed content types.
async fn process_text_content(
    uri: http::Uri,
    data_store: Data<String>,
    Text(content): Text,
) -> Result<()> {
    let url = uri.to_string();
    tracing::info!("Processing text content from: {}", url);

    let content_length = content.len();
    let line_count = content.lines().count();

    // Try to detect if content might be JSON
    let might_be_json =
        content.trim_start().starts_with('{') || content.trim_start().starts_with('[');

    let content_type = if might_be_json {
        "JSON-like text"
    } else if content.contains("html") || content.contains("HTML") {
        "HTML-like text"
    } else {
        "Plain text"
    };

    tracing::info!(
        "Content analysis: {} bytes, {} lines, type: {}",
        content_length,
        line_count,
        content_type
    );

    data_store
        .write(format!(
            "{content_type}: {url} ({content_length} bytes, {line_count} lines)"
        ))
        .await?;

    Ok(())
}

/// Handler for processing JSON APIs with endpoint discovery.
///
/// This handler demonstrates extracting URLs from JSON responses and
/// queueing additional API requests for discovered endpoints, enabling
/// automated API exploration and link following.
async fn process_json_with_discovery(
    uri: http::Uri,
    queue: Queue,
    data_store: Data<String>,
    Json(data): Json<HashMap<String, Value>>,
) -> Result<()> {
    let url = uri.to_string();
    tracing::info!("Processing JSON with endpoint discovery from: {}", url);

    data_store
        .write(format!(
            "Discovered API endpoint: {} with {} fields",
            url,
            data.len()
        ))
        .await?;

    // Look for common URL patterns in the JSON response
    let mut discovered_endpoints = 0;

    for value in data.values() {
        if let Some(link_url) = value.as_str()
            && link_url.starts_with("http")
            && discovered_endpoints < 2
        {
            queue
                .follow(Seed::get(link_url)?.with_tag("dynamic_json"))
                .await?;
            discovered_endpoints += 1;
            tracing::info!("Discovered endpoint: {}", link_url);
        }
    }

    if discovered_endpoints > 0 {
        data_store
            .write(format!(
                "Discovered {discovered_endpoints} additional endpoints from {url}"
            ))
            .await?;
    }

    Ok(())
}

/// Handler for processing standard JSON API responses.
///
/// This handler demonstrates processing well-structured JSON responses
/// and extracting key-value data from common API response formats.
async fn process_standard_json(
    uri: http::Uri,
    data_store: Data<String>,
    Json(data): Json<Value>,
) -> Result<()> {
    let url = uri.to_string();
    tracing::info!("Processing standard JSON from: {}", url);

    // Process common JSON structures
    if let Some(obj) = data.as_object() {
        let mut extracted_info = Vec::new();

        // Look for common fields
        for (key, value) in obj.iter().take(10) {
            let value_summary = match value {
                Value::String(s) => format!("\"{}\"", s.chars().take(50).collect::<String>()),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Array(arr) => format!("Array[{}]", arr.len()),
                Value::Object(obj) => format!("Object{{{}}}", obj.len()),
                Value::Null => "null".to_string(),
            };
            extracted_info.push(format!("{key}: {value_summary}"));
        }

        for info in extracted_info {
            data_store.write(info).await?;
        }
    }

    data_store
        .write(format!("Processed standard JSON from {url}"))
        .await?;

    Ok(())
}

/// Creates the crawler with its JSON processing routes and initial requests.
fn create_crawler() -> Result<Crawler<HttpClient>> {
    let router = Router::new()
        .route("structured_json", process_structured_json)
        .route("dynamic_json", process_dynamic_json)
        .route("text_content", process_text_content)
        .route("json_discovery", process_json_with_discovery)
        .route("standard_json", process_standard_json);

    // The same endpoint is seeded twice under different tags, so the frontier
    // retries but does not deduplicate.
    let seeds = [
        ("standard_json", "https://httpbin.org/json"),
        ("dynamic_json", "https://httpbin.org/uuid"),
        ("text_content", "https://httpbin.org/robots.txt"),
        ("structured_json", "https://httpbin.org/json"),
    ]
    .into_iter()
    .map(|(tag, url)| Seed::get(url).map(|seed| seed.with_tag(tag)))
    .collect::<Result<Vec<_>>>()?;

    let crawler = Crawler::new(HttpClient::default(), router)
        .with_frontier(InMemFrontier::fifo().retry(2))
        .with_dataset(InMemDataset::<User>::new())
        .with_dataset(InMemDataset::<ApiMetadata>::new())
        .with_dataset(InMemDataset::<String>::new())
        .with_seeds(seeds);

    Ok(crawler)
}

/// Displays the processing results from all datasets.
async fn display_results(crawler: &Crawler<HttpClient>) -> Result<()> {
    // Display users
    let user_dataset = crawler.dataset::<User>()?;
    let users = user_dataset.read_all().await?;
    tracing::info!("Processed {} users:", users.len());
    for user in users {
        tracing::info!("  {}: {} from {}", user.name, user.email, user.source_url);
    }

    // Display API metadata
    let metadata_dataset = crawler.dataset::<ApiMetadata>()?;
    let metadata = metadata_dataset.read_all().await?;
    tracing::info!("Collected metadata from {} APIs:", metadata.len());
    for meta in metadata {
        tracing::info!(
            "  {}: {} fields, {} bytes",
            meta.endpoint,
            meta.fields_count,
            meta.response_size
        );
    }

    // Display text processing results
    let string_dataset = crawler.dataset::<String>()?;
    let strings = string_dataset.read_all().await?;
    tracing::info!("Collected {} processing results", strings.len());
    for (i, entry) in strings.iter().enumerate() {
        tracing::info!("  {}: {}", i + 1, entry);
    }

    Ok(())
}

/// Count the number of fields in a JSON value.
fn count_json_fields(value: &Value) -> usize {
    match value {
        Value::Object(obj) => obj.len(),
        Value::Array(arr) => arr.len(),
        _ => 1,
    }
}

/// Format a JSON value for logging.
fn format_json_value(value: &Value) -> String {
    match value {
        Value::String(s) => format!("\"{}\"", s.chars().take(50).collect::<String>()),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(arr) => format!("Array[{}]", arr.len()),
        Value::Object(obj) => format!("Object{{{}}}", obj.len()),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize structured logging
    tracing_subscriber::fmt().init();

    tracing::info!("Starting Spire API Endpoints Example");

    let crawler = create_crawler()?;

    tracing::info!("Starting JSON API processing");
    let stats = crawler.run().await?;
    tracing::info!(?stats, "JSON API processing finished");

    display_results(&crawler).await?;

    tracing::info!("API endpoints example completed successfully");
    Ok(())
}
