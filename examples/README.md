# Spire Examples

This directory contains examples demonstrating the core functionality of the Spire web scraping and crawling framework. Each example showcases different aspects of the framework's capabilities and serves as both documentation and integration tests.

## Available Examples

### Basic Usage

**Location**: `examples/basic_usage/`

Demonstrates fundamental Spire functionality using the HTTP backend with the reqwest implementation.

**Key Features**: This example covers HTTP client setup and configuration, request routing with tag-based handlers, text and JSON data extraction using built-in extractors, request queue management and dataset storage, structured error handling with Spire Result types, and tracing integration for observability.

**Learning Outcomes**: You'll learn about core Spire architecture and request flow, handler function patterns and extractors, and basic data processing and storage workflows.

```rust
use spire::prelude::*;

async fn scrape_html_page(
    Text(html): Text,
    request: Request,
    mut data_sink: DataSink<String>,
) -> Result<()> {
    let url = request.uri().to_string();
    data_sink.push(format!("Scraped: {}", url)).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let router = Router::new()
        .route(Tag::new("html_page"), scrape_html_page);

    let client = Client::new(HttpClient::default(), router)
        .with_request_queue(InMemDataset::stack())
        .with_dataset(InMemDataset::<String>::new());

    client.queue()
        .push(Tag::new("html_page"), "https://example.com")
        .await?;

    client.run().await
}
```

### Chrome WebDriver

**Location**: `examples/chrome_webdriver/`

Demonstrates browser automation using the thirtyfour backend for JavaScript-heavy sites and dynamic content.

**Key Features**: This example demonstrates browser backend configuration with Chrome WebDriver, connection pool management for concurrent operations, dynamic page interaction and JavaScript execution, form filling and submission workflows, element selection and manipulation, and single-page application content extraction.

**Learning Outcomes**: You'll master browser automation patterns with Spire, handling dynamic and JavaScript-rendered content, WebDriver configuration and resource management, and advanced element interaction techniques.

**Requirements**: You need Chrome browser installed and accessible, with ChromeDriver automatically managed by thirtyfour.

### Tower Layers

**Location**: `examples/tower_layers/`

Shows integration with the Tower middleware ecosystem for building robust, production-ready scraping systems.

**Key Features**: This example showcases custom middleware layer implementation, built-in Tower middleware composition including timeout, retry, and rate limiting, request and response processing through middleware stack, error handling strategies across layers, observability and metrics collection, and resilient scraping architecture patterns.

**Learning Outcomes**: You'll understand composable middleware design with Tower, building fault-tolerant scraping systems, custom middleware development patterns, and production-ready error handling and recovery.

## Running Examples

Navigate to any example directory and execute with cargo:

```bash
# Basic HTTP scraping example
cd examples/basic_usage
cargo run

# Browser automation example
cd examples/chrome_webdriver
cargo run

# Middleware integration example
cd examples/tower_layers
cargo run
```

## Example Structure

Each example follows a consistent structure with comprehensive documentation that includes detailed comments explaining concepts and patterns. They use real-world scenarios with httpbin.org for reliable testing endpoints, demonstrate proper error handling using Spire's Result types, include integrated tracing for debugging and monitoring, and show production patterns with scalable and maintainable code organization.

## Dependencies

Examples are self-contained with their own `Cargo.toml` files. The `basic_usage` example focuses on core HTTP functionality with reqwest backend, `chrome_webdriver` demonstrates browser automation with thirtyfour backend, and `tower_layers` shows middleware integration with Tower ecosystem.

## Development Guidelines

These examples serve as living documentation and integration tests. When contributing, ensure examples compile and run successfully, include comprehensive inline documentation, follow established architectural patterns, verify examples work with current framework versions, and maintain consistent style and structure across examples.

## Troubleshooting

**Chrome WebDriver Issues**: Ensure Chrome browser is installed and accessible, note that ChromeDriver is automatically managed by thirtyfour, and try running without `--headless` flag for debugging.

**Network Errors**: Examples use httpbin.org which should be reliable, so check network connectivity and firewall settings, and enable debug logging with `RUST_LOG=debug cargo run`.

## See Also

- [Main Documentation](../README.md) - Framework overview and getting started guide
- [Core Crate](../crates/spire/) - Core framework implementation
- [Contributing Guide](../CONTRIBUTING.md) - Development and contribution guidelines
- [API Documentation](https://docs.rs/spire) - Comprehensive API reference