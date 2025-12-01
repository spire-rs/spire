# spire-thirtyfour

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]
[![Crate Coverage][coverage-badge]][coverage-url]

**Check out other `spire` projects [here](https://github.com/spire-rs).**

[action-badge]: https://img.shields.io/github/actions/workflow/status/spire-rs/spire/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/spire-rs/spire/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/spire-thirtyfour.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/spire-thirtyfour
[docs-badge]: https://img.shields.io/docsrs/spire-thirtyfour?logo=Docs.rs&style=flat-square
[docs-url]: http://docs.rs/spire-thirtyfour
[coverage-badge]: https://img.shields.io/codecov/c/github/spire-rs/spire?logo=codecov&logoColor=white&style=flat-square
[coverage-url]: https://app.codecov.io/gh/spire-rs/spire

Browser automation backend for the spire web scraping framework, powered by [thirtyfour](https://github.com/stevepryde/thirtyfour).

## Overview

`spire-thirtyfour` provides a browser automation backend implementation for spire using the thirtyfour WebDriver library. This backend enables JavaScript-heavy website scraping by controlling real browsers through the WebDriver protocol.

## Key Features

- **Full Browser Support**: Chrome, Firefox, Safari, Edge via WebDriver
- **JavaScript Execution**: Handle dynamic content and SPA applications
- **Connection Pooling**: Efficient browser instance management with deadpool
- **Element Interaction**: Click, type, scroll, and interact with page elements
- **Screenshot Capture**: Take screenshots for debugging or archival
- **Network Interception**: Monitor and modify network requests
- **Mobile Emulation**: Simulate mobile devices and different screen sizes

## Usage

This crate is typically not used directly. Instead, enable the `thirtyfour` feature in the main `spire` crate:

```toml
[dependencies]
spire = { version = "0.2.0", features = ["thirtyfour"] }
```

Then use the browser automation backend in your spire applications:

```rust
use spire::prelude::*;
use spire::extract::Text;
use spire::context::{RequestQueue, Tag};
use spire::thirtyfour_backend::BrowserPool;
use spire::dataset::InMemDataset;

async fn handler(
    Text(html): Text,
    queue: RequestQueue,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Scraped {} bytes from rendered page", html.len());
    
    // You can also access the WebDriver directly for complex interactions
    // let driver = extract::Driver(driver);
    // driver.find_element(By::Css(".load-more")).await?.click().await?;
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let router = Router::new()
        .route(Tag::new("spa"), handler);

    // Create browser pool backend
    let backend = BrowserPool::builder().build();
    
    let client = Client::new(backend, router)
        .with_request_queue(InMemDataset::stack())
        .with_dataset(InMemDataset::<String>::new());

    client.queue()
        .push(Tag::new("spa"), "https://spa-example.com")
        .await?;

    client.run().await?;
    Ok(())
}
```

## Configuration

You can customize the browser pool backend:

```rust
use spire::thirtyfour_backend::BrowserPool;
use thirtyfour::DesiredCapabilities;

// Create a custom browser pool
let backend = BrowserPool::builder()
    .max_size(10)  // Maximum 10 browser instances
    .webdriver_url("http://localhost:4444")  // Custom WebDriver URL
    .capabilities(DesiredCapabilities::chrome())
    .headless(true)  // Run in headless mode
    .build();
```

## Prerequisites

You'll need a WebDriver server running. For Chrome:

```bash
# Install ChromeDriver
brew install chromedriver  # macOS
# or download from https://chromedriver.chromium.org/

# Run ChromeDriver
chromedriver --port=4444
```

For other browsers, see the [thirtyfour documentation](https://docs.rs/thirtyfour/) for setup instructions.

## Advanced Usage

Access the WebDriver directly for complex interactions:

```rust
use spire::extract::Driver;
use thirtyfour::By;

async fn interactive_handler(
    Driver(driver): Driver,
    queue: RequestQueue,
) -> Result<(), Box<dyn std::error::Error>> {
    // Wait for an element to appear
    let element = driver.query(By::Css("#load-more-btn"))
        .wait(Duration::from_secs(10), Duration::from_millis(500))
        .first().await?;
    
    // Click the button
    element.click().await?;
    
    // Wait for content to load
    driver.implicitly_wait(Duration::from_secs(2)).await?;
    
    // Queue more pages
    let next_links = driver.find_elements(By::Css("a.next-page")).await?;
    for link in next_links {
        if let Ok(href) = link.get_attribute("href").await {
            if let Some(url) = href {
                queue.push(Tag::new("spa"), url).await?;
            }
        }
    }
    
    Ok(())
}
```

## Direct Usage

If you need to use this crate directly (for custom backend implementations):

```toml
[dependencies]
spire-thirtyfour = "0.2.0"
```

```rust
use spire_thirtyfour::BrowserPool;
use spire_core::backend::Backend;

let backend = BrowserPool::builder().build();
let client = backend.connect().await?;
```

## License

This project is licensed under the MIT License - see the [LICENSE](../LICENSE) file for details.