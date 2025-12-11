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
use spire::context::RequestQueue;
use spire::dataset::future::Data;
use spire::dataset::{Dataset, DatasetBatchExt, InMemDataset};
use spire::extract::{Json, Text};
use spire::{Client, HttpClient, Result, Router, http};

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
    let user = User::from_json(json_data, url.clone())?;

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

    let json_string = serde_json::to_string(&data).map_err(|e| {
        spire::Error::with_source(
            spire::ErrorKind::Context,
            "failed to serialize JSON",
            Box::new(e),
        )
    })?;

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
            "{}: {} ({} bytes, {} lines)",
            content_type, url, content_length, line_count
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
    queue: RequestQueue,
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

    for (_key, value) in &data {
        if let Some(link_url) = value.as_str() {
            if link_url.starts_with("http") && discovered_endpoints < 2 {
                queue.append_with_tag("dynamic_json", link_url).await?;
                discovered_endpoints += 1;
                tracing::info!("Discovered endpoint: {}", link_url);
            }
        }
    }

    if discovered_endpoints > 0 {
        data_store
            .write(format!(
                "Discovered {} additional endpoints from {}",
                discovered_endpoints, url
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
            extracted_info.push(format!("{}: {}", key, value_summary));
        }

        for info in extracted_info {
            data_store.write(info).await?;
        }
    }

    data_store
        .write(format!("Processed standard JSON from {}", url))
        .await?;

    Ok(())
}

/// Creates and configures the HTTP client with JSON processing routes.
fn create_client() -> Client<HttpClient> {
    let router = Router::new()
        .route("structured_json", process_structured_json)
        .route("dynamic_json", process_dynamic_json)
        .route("text_content", process_text_content)
        .route("json_discovery", process_json_with_discovery)
        .route("standard_json", process_standard_json);

    Client::new(HttpClient::default(), router)
        .with_request_queue(InMemDataset::stack())
        .with_dataset(InMemDataset::<User>::new())
        .with_dataset(InMemDataset::<ApiMetadata>::new())
        .with_dataset(InMemDataset::<String>::new())
}

/// Queues initial API requests for processing.
async fn queue_initial_requests(queue: &RequestQueue) -> Result<()> {
    // JSON API with structured data
    queue
        .append_with_tag("standard_json", "https://httpbin.org/json")
        .await?;

    // UUID endpoint for dynamic processing
    queue
        .append_with_tag("dynamic_json", "https://httpbin.org/uuid")
        .await?;

    // Text content for mixed processing
    queue
        .append_with_tag("text_content", "https://httpbin.org/robots.txt")
        .await?;

    // Another JSON endpoint for user data
    queue
        .append_with_tag("structured_json", "https://httpbin.org/json")
        .await?;

    Ok(())
}

/// Displays the processing results from all datasets.
async fn display_results(client: &Client<HttpClient>) -> Result<()> {
    // Display users
    let user_dataset = client.dataset::<User>();
    let users = user_dataset.read_all().await?;
    tracing::info!("Processed {} users:", users.len());
    for user in users {
        tracing::info!("  {}: {} from {}", user.name, user.email, user.source_url);
    }

    // Display API metadata
    let metadata_dataset = client.dataset::<ApiMetadata>();
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
    let string_dataset = client.dataset::<String>();
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

    // Create client and router
    let client = create_client();

    // Queue initial API requests
    let queue = client.request_queue();
    queue_initial_requests(&queue).await?;

    tracing::info!("Starting JSON API processing");

    // Execute the API processing
    client.run().await?;

    // Display results
    display_results(&client).await?;

    tracing::info!("API endpoints example completed successfully");
    Ok(())
}
