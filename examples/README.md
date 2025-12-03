# Spire Examples

This directory contains examples demonstrating the core functionality of the Spire web scraping and crawling framework. Each example showcases different aspects of the framework's capabilities and serves as both documentation and integration tests.

## Available Examples

### Basic Usage

**Location**: `examples/basic_usage/`

Demonstrates fundamental Spire functionality using the HTTP backend with the reqwest implementation.

**Key Features**:
- HTTP client setup and configuration
- Request routing with tag-based handlers
- Text and JSON data extraction using built-in extractors
- Request queue management and dataset storage
- Structured error handling with Spire Result types
- Tracing integration for observability

**Learning Outcomes**:
- Core Spire architecture and request flow
- Handler function patterns and extractors
- Basic data processing and storage workflows

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

**Key Features**:
- Browser backend configuration with Chrome WebDriver
- Connection pool management for concurrent operations
- Dynamic page interaction and JavaScript execution
- Form filling and submission workflows
- Element selection and manipulation
- Single-page application (SPA) content extraction

**Learning Outcomes**:
- Browser automation patterns with Spire
- Handling dynamic and JavaScript-rendered content
- WebDriver configuration and resource management
- Advanced element interaction techniques

**Requirements**:
- Chrome browser installed and accessible
- ChromeDriver (automatically managed by thirtyfour)

### Tower Layers

**Location**: `examples/tower_layers/`

Shows integration with the Tower middleware ecosystem for building robust, production-ready scraping systems.

**Key Features**:
- Custom middleware layer implementation
- Built-in Tower middleware composition (timeout, retry, rate limiting)
- Request and response processing through middleware stack
- Error handling strategies across layers
- Observability and metrics collection
- Resilient scraping architecture patterns

**Learning Outcomes**:
- Composable middleware design with Tower
- Building fault-tolerant scraping systems
- Custom middleware development patterns
- Production-ready error handling and recovery

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

Each example follows a consistent structure:

- **Comprehensive documentation**: Detailed comments explaining concepts and patterns
- **Real-world scenarios**: Uses httpbin.org for reliable testing endpoints
- **Error handling**: Demonstrates proper use of Spire's Result types
- **Observability**: Integrated tracing for debugging and monitoring
- **Production patterns**: Shows scalable and maintainable code organization

## Dependencies

Examples are self-contained with their own `Cargo.toml` files:

- `basic_usage`: Core HTTP functionality with reqwest backend
- `chrome_webdriver`: Browser automation with thirtyfour backend
- `tower_layers`: Middleware integration with Tower ecosystem

## Development Guidelines

These examples serve as living documentation and integration tests. When contributing:

1. **Functionality**: Ensure examples compile and run successfully
2. **Documentation**: Include comprehensive inline documentation
3. **Patterns**: Follow established architectural patterns
4. **Testing**: Verify examples work with current framework versions
5. **Consistency**: Maintain consistent style and structure across examples

## Troubleshooting

**Chrome WebDriver Issues**:
- Ensure Chrome browser is installed and accessible
- ChromeDriver is automatically managed by thirtyfour
- Try running without `--headless` flag for debugging

**Network Errors**:
- Examples use httpbin.org which should be reliable
- Check network connectivity and firewall settings
- Enable debug logging with `RUST_LOG=debug cargo run`

## See Also

- [Main Documentation](../README.md) - Framework overview and getting started guide
- [Core Crate](../crates/spire/) - Core framework implementation
- [Contributing Guide](../CONTRIBUTING.md) - Development and contribution guidelines
- [API Documentation](https://docs.rs/spire) - Comprehensive API reference