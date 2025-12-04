# Examples

This directory contains examples demonstrating the core functionality of the Spire web scraping and crawling framework. Each example showcases different aspects of the framework's capabilities and serves as both documentation and integration tests.

## Available Examples

### Basic Usage

**Location**: `examples/basic_usage/`

Demonstrates fundamental Spire functionality including custom data types, HTML parsing, and text processing.

**Key Features**: 
- HTTP client setup with reqwest backend
- Custom data structures for dataset storage
- HTML content extraction using the Html extractor
- Text content processing and analysis
- Tag-based routing using Into<Tag>
- Jiff timestamps and proper URL types
- Request queue management and link discovery

**Learning Outcomes**: Core Spire architecture, handler patterns with extractors, custom dataset usage, and basic HTML parsing workflows.

### API Endpoints

**Location**: `examples/api_endpoints/`

Shows how to process JSON API responses with structured and dynamic JSON handling, demonstrating multiple dataset management.

**Key Features**:
- Structured JSON API processing with custom data types
- Dynamic JSON response handling and analysis
- Text content processing and analysis
- API endpoint discovery and link following
- Multiple dataset types for different data structures
- Proper error handling and data validation

**Learning Outcomes**: JSON API integration patterns, structured vs. dynamic JSON processing, multi-dataset management, and API endpoint discovery.



### Chrome WebDriver

**Location**: `examples/chrome_webdriver/`

Demonstrates browser automation using the thirtyfour backend for JavaScript-heavy sites and dynamic content rendering.

**Key Features**:
- Browser backend configuration with Chrome WebDriver
- Basic page rendering and content extraction
- Dynamic content loading with extended wait patterns
- WebDriver integration and error handling
- Structured data extraction from browser-rendered pages
- Graceful error handling when WebDriver is unavailable

**Learning Outcomes**: Browser automation patterns, handling dynamic content, WebDriver integration, and browser-based scraping workflows.

**Requirements**: Chrome browser installed (ChromeDriver automatically managed by thirtyfour).



## Running Examples

Navigate to any example directory and execute with cargo:

```bash
# Basic usage example
cd examples/basic_usage
cargo run

# API endpoints processing
cd examples/api_endpoints
cargo run

# Browser automation
cd examples/chrome_webdriver
cargo run
```

## Example Structure

Each example follows a consistent structure:

- **Comprehensive documentation** with detailed comments explaining concepts and patterns
- **Real-world scenarios** using httpbin.org for reliable testing endpoints  
- **Proper error handling** using Spire's Result types
- **Integrated tracing** for debugging and monitoring
- **Production patterns** with scalable and maintainable code organization

## Dependencies

Examples are self-contained with their own `Cargo.toml` files and organized dependency categories:

- **Async runtime and Spire framework**: Core tokio and spire dependencies
- **Serialization and JSON/data processing**: Data handling libraries
- **Data types**: Proper URL and timestamp handling with jiff
- **Observability**: Tracing and logging support

## Development Guidelines

These examples serve as living documentation and integration tests. When contributing:

- Ensure examples compile and run successfully
- Include comprehensive inline documentation  
- Follow established architectural patterns
- Verify examples work with current framework versions
- Maintain consistent style and structure across examples

## Troubleshooting

**Chrome WebDriver Issues**: 
- Ensure Chrome browser is installed and accessible
- ChromeDriver is automatically managed by thirtyfour
- Verify WebDriver server is running on port 4444 for full browser automation
- Example demonstrates graceful error handling when WebDriver is unavailable

**Network Errors**:
- Examples use httpbin.org which should be reliable
- Check network connectivity and firewall settings  
- Enable debug logging with `RUST_LOG=debug cargo run`

**Response Body Issues**:
- Current reqwest backend implementation has limitations with response body extraction
- Examples demonstrate proper patterns and error handling
- Framework architecture and example structure work correctly

## See Also

- [Main Documentation](../README.md) - Framework overview and getting started guide
- [Core Crate](../spire/) - Core framework implementation  
- [Contributing Guide](../CONTRIBUTING.md) - Development and contribution guidelines
- [API Documentation](https://docs.rs/spire) - Comprehensive API reference