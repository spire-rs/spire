//! Chrome WebDriver example demonstrating browser automation with Spire.
//!
//! This example shows how to:
//! - Set up Chrome WebDriver with Spire
//! - Use the thirtyfour backend for browser automation
//! - Navigate to pages and wait for JavaScript rendering
//! - Extract data from dynamically loaded content
//! - Handle WebDriver sessions and cleanup
//!
//! Prerequisites:
//! - ChromeDriver must be installed and available in PATH
//! - Chrome browser must be installed
//! - Run: `cargo run --example chrome_webdriver --features thirtyfour`

use spire::prelude::*;
use spire::thirtyfour_backend::{BrowserPool, WebDriverBackend};
use spire::dataset::InMemDataset;
use spire::extract::{State, WebDriver};
use thirtyfour::prelude::*;
use std::time::Duration;

#[derive(Debug, Clone)]
struct DynamicContent {
    title: String,
    text: String,
    timestamp: String,
}

/// Extract content that requires JavaScript execution
async fn extract_dynamic_content(
    WebDriver(driver): WebDriver,
    State(mut dataset): State<InMemDataset<DynamicContent>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait for page to fully load
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Find and extract the page title
    let title = driver.title().await?;

    // Extract text content from specific elements
    let text_elements = driver.find_all(By::Tag("p")).await?;
    let mut combined_text = String::new();

    for element in text_elements {
        let text = element.text().await?;
        if !text.trim().is_empty() {
            combined_text.push_str(&text);
            combined_text.push('\n');
        }
    }

    // Try to get timestamp if available (common in SPAs)
    let timestamp = match driver.find(By::Id("timestamp")).await {
        Ok(element) => element.text().await.unwrap_or_else(|_| "N/A".to_string()),
        Err(_) => chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
    };

    let content = DynamicContent {
        title,
        text: combined_text.trim().to_string(),
        timestamp,
    };

    dataset.insert(content.clone()).await?;
    println!("Extracted dynamic content: {}", content.title);

    Ok(())
}

/// Example of interacting with page elements
async fn interact_with_page(
    WebDriver(driver): WebDriver,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Example: Fill out a form if present
    if let Ok(input) = driver.find(By::Tag("input")).await {
        input.send_keys("Spire WebDriver Test").await?;
        println!("Filled input field");
    }

    // Example: Click a button if present
    if let Ok(button) = driver.find(By::Tag("button")).await {
        button.click().await?;
        println!("Clicked button");

        // Wait for any resulting page changes
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    // Example: Take a screenshot
    let screenshot = driver.screenshot_as_png().await?;
    println!("Screenshot taken ({} bytes)", screenshot.len());

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::init();

    println!("Starting Chrome WebDriver example...");

    // Create Chrome capabilities
    let mut caps = DesiredCapabilities::chrome();
    caps.add_chrome_arg("--headless")?; // Run in headless mode
    caps.add_chrome_arg("--no-sandbox")?;
    caps.add_chrome_arg("--disable-dev-shm-usage")?;
    caps.add_chrome_arg("--disable-gpu")?;
    caps.add_chrome_arg("--window-size=1920,1080")?;

    // Create browser pool for managing WebDriver sessions
    let pool = BrowserPool::new("http://localhost:9515", caps, 2).await?;

    // Create the WebDriver backend
    let backend = WebDriverBackend::new(pool);

    // Create dataset for storing results
    let dataset = InMemDataset::<DynamicContent>::new();

    // Build the scraper
    let scraper = Scraper::builder()
        .with_backend(backend)
        .with_state(dataset.clone())
        .build();

    // URLs to test (using httpbin for reliable testing)
    let test_urls = vec![
        "https://httpbin.org/html",
        "https://httpbin.org/forms/post",
    ];

    println!("Processing URLs with Chrome WebDriver...");

    for url in test_urls {
        println!("Processing: {}", url);

        match scraper
            .get(&url)
            .extract_with(extract_dynamic_content)
            .extract_with(interact_with_page)
            .send()
            .await
        {
            Ok(_) => println!("✓ Successfully processed {}", url),
            Err(e) => eprintln!("✗ Failed to process {}: {}", url, e),
        }

        // Small delay between requests
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    // Display results
    let results = dataset.get_all().await?;
    println!("\nExtracted {} items:", results.len());

    for (i, content) in results.iter().enumerate() {
        println!("{}. Title: {}", i + 1, content.title);
        println!("   Timestamp: {}", content.timestamp);
        println!("   Text length: {} chars", content.text.len());
        if !content.text.is_empty() {
            let preview = if content.text.len() > 100 {
                format!("{}...", &content.text[..97])
            } else {
                content.text.clone()
            };
            println!("   Preview: {}", preview.replace('\n', " "));
        }
        println!();
    }

    println!("Chrome WebDriver example completed!");
    Ok(())
}

/// Configuration for different browser setups
#[allow(dead_code)]
fn chrome_headful_caps() -> Result<DesiredCapabilities, WebDriverError> {
    let mut caps = DesiredCapabilities::chrome();
    caps.add_chrome_arg("--window-size=1920,1080")?;
    caps.add_chrome_arg("--start-maximized")?;
    Ok(caps)
}

#[allow(dead_code)]
fn chrome_mobile_caps() -> Result<DesiredCapabilities, WebDriverError> {
    let mut caps = DesiredCapabilities::chrome();
    caps.add_chrome_arg("--headless")?;
    caps.add_chrome_arg("--user-agent=Mozilla/5.0 (iPhone; CPU iPhone OS 14_0 like Mac OS X) AppleWebKit/605.1.15")?;
    caps.add_chrome_arg("--window-size=375,812")?; // iPhone size
    Ok(caps)
}
