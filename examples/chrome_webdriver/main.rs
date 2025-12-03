//! Chrome WebDriver example demonstrating browser automation with Spire.
//!
//! This example showcases the browser automation capabilities of Spire using
//! the thirtyfour backend with Chrome WebDriver. It demonstrates:
//! - Configuring browser backends with WebDriver settings
//! - Managing browser connection pools for concurrent operations
//! - Interacting with dynamic web pages and JavaScript content
//! - Extracting data from browser-rendered pages
//! - Handling different types of web interactions (navigation, forms, SPAs)
//!
//! The example uses httpbin.org endpoints to demonstrate real browser
//! interactions in a controlled environment without depending on
//! external services that might change.
//!
//! Prerequisites:
//! - Chrome browser installed and accessible
//! - ChromeDriver (automatically managed by thirtyfour)

use std::time::Duration;

use spire::extract::driver::View;
use spire::prelude::*;
use tracing::{error, info, warn};

/// Handler for processing dynamic pages that require JavaScript execution.
///
/// This handler demonstrates:
/// - Waiting for page content to load completely
/// - Extracting page metadata (title, headings)
/// - Finding and processing page elements
/// - Queueing additional pages based on discovered links
async fn scrape_dynamic_page(
    view: View,
    mut queue: RequestQueue,
    data: Data<String>,
) -> Result<()> {
    let current_url = view.current_url().await?;
    info!("Processing dynamic page: {}", current_url);

    // Allow time for JavaScript and dynamic content to load
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Extract and store page title
    let title = view.title().await?;
    info!("Page title: {}", title);
    data.write(format!("Title from {}: {}", current_url, title))
        .await?;

    // Find and extract all heading elements
    if let Ok(headings) = view.find_elements_by_tag_name("h1").await {
        info!("Found {} H1 elements", headings.len());

        for (index, heading) in headings.iter().enumerate() {
            let heading_text = heading.text().await?;
            if !heading_text.is_empty() {
                info!("H1 #{}: {}", index + 1, heading_text);
                data.write(format!("Heading: {}", heading_text)).await?;
            }
        }
    }

    // Discover and queue additional links for processing
    if let Ok(links) = view.find_elements_by_tag_name("a").await {
        info!("Found {} links on page", links.len());

        let mut queued_count = 0;
        for link in links.iter().take(3) {
            if let Ok(href) = link.get_attribute("href").await {
                if let Some(url) = href {
                    if url.starts_with("http") && queued_count < 2 {
                        info!("Queueing linked page: {}", url);
                        queue.push(Tag::new("dynamic_page"), url).await?;
                        queued_count += 1;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Handler for processing and interacting with web forms.
///
/// This handler demonstrates:
/// - Finding and analyzing form elements
/// - Extracting form field information
/// - Filling out forms programmatically
/// - Submitting forms and handling navigation
async fn scrape_form_page(view: View, data: Data<String>) -> Result<()> {
    let current_url = view.current_url().await?;
    info!("Processing form page: {}", current_url);

    // Analyze all forms on the page
    if let Ok(forms) = view.find_elements_by_tag_name("form").await {
        info!("Found {} forms on page", forms.len());

        for (index, _form) in forms.iter().enumerate() {
            let form_info = format!("Form #{} found on {}", index + 1, current_url);
            info!("{}", form_info);
            data.write(form_info).await?;
        }
    }

    // Catalog all input fields and their types
    if let Ok(inputs) = view.find_elements_by_tag_name("input").await {
        info!("Found {} input fields", inputs.len());

        for input in inputs.iter() {
            if let (Ok(Some(input_type)), Ok(Some(name))) = (
                input.get_attribute("type").await,
                input.get_attribute("name").await,
            ) {
                let field_info = format!("Input field '{}' (type: {})", name, input_type);
                info!("{}", field_info);
                data.write(field_info).await?;
            }
        }
    }

    // Attempt to interact with a search form if present
    if let Ok(search_input) = view.find_element_by_name("q").await {
        info!("Found search input field, demonstrating form interaction");

        // Clear any existing content and enter search text
        search_input.clear().await?;
        search_input
            .send_keys("Spire web scraping framework")
            .await?;

        // Look for and click submit button
        let submit_selector = "input[type='submit'], button[type='submit']";
        if let Ok(submit_button) = view.find_element_by_css_selector(submit_selector).await {
            info!("Submitting form");
            submit_button.click().await?;

            // Wait for potential page navigation
            tokio::time::sleep(Duration::from_secs(3)).await;

            let new_url = view.current_url().await?;
            let navigation_info = format!("Form submitted, navigated to: {}", new_url);
            info!("{}", navigation_info);
            data.write(navigation_info).await?;
        }
    }

    Ok(())
}

/// Handler for processing single-page applications and JavaScript-heavy content.
///
/// This handler demonstrates:
/// - Waiting for asynchronous content loading
/// - Executing JavaScript in the browser context
/// - Polling for dynamically loaded elements
/// - Extracting content from JavaScript-rendered pages
async fn scrape_spa_page(view: View, data: Data<String>) -> Result<()> {
    let current_url = view.current_url().await?;
    info!("Processing SPA page: {}", current_url);

    // Allow extended time for JavaScript frameworks to initialize
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Execute JavaScript to extract content that might not be accessible via DOM
    let js_script = "return document.querySelector('body').innerText.substring(0, 200);";
    let js_result: String = view
        .execute_script(js_script, vec![])
        .await
        .unwrap_or_default();

    if !js_result.is_empty() {
        info!(
            "Extracted JavaScript content: {} characters",
            js_result.len()
        );
        data.write(format!("JS Content: {}", js_result)).await?;
    }

    // Poll for dynamically loaded content with timeout
    let max_attempts = 10;
    let mut attempt = 0;

    while attempt < max_attempts {
        // Look for common patterns in dynamic content
        let dynamic_selectors = "[data-testid], .dynamic-content, .loaded, .spa-content";

        if let Ok(dynamic_elements) = view.find_elements_by_css_selector(dynamic_selectors).await {
            if !dynamic_elements.is_empty() {
                info!(
                    "Found {} dynamic elements after {} attempts",
                    dynamic_elements.len(),
                    attempt + 1
                );

                // Process the first few dynamic elements
                for (index, element) in dynamic_elements.iter().enumerate().take(5) {
                    let element_text = element.text().await.unwrap_or_default();
                    if !element_text.is_empty() {
                        let preview = element_text.chars().take(100).collect::<String>();
                        info!("Dynamic element #{}: {}", index + 1, preview);

                        let content_info = format!(
                            "Dynamic content: {}",
                            element_text.chars().take(200).collect::<String>()
                        );
                        data.write(content_info).await?;
                    }
                }
                break;
            }
        }

        // Wait before next attempt
        tokio::time::sleep(Duration::from_millis(500)).await;
        attempt += 1;
    }

    if attempt >= max_attempts {
        warn!("No dynamic content found after {} attempts", max_attempts);
    }

    Ok(())
}

/// Error handler for browser automation failures.
///
/// This handler demonstrates:
/// - Categorizing different types of browser errors
/// - Logging appropriate error information
/// - Handling navigation and WebDriver-specific errors
async fn handle_browser_error(request: Request, error: BrowserError) -> Result<()> {
    let url = request.uri().to_string();

    match error {
        BrowserError::Navigation(nav_error) => match nav_error {
            NavigationErrorType::Timeout => {
                warn!("Navigation timeout for: {}", url);
            }
            NavigationErrorType::NetworkError => {
                error!("Network error navigating to: {}", url);
            }
            _ => {
                error!("Navigation error for {}: {:?}", url, nav_error);
            }
        },
        _ => {
            error!("Browser error for {}: {:?}", url, error);
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize structured logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();

    info!("Starting Spire Chrome WebDriver Example");

    // Configure Chrome WebDriver with appropriate settings
    let webdriver_config = WebDriverConfigBuilder::default()
        .binary_path("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome") // macOS path
        .args(vec![
            "--headless=new".to_string(),
            "--no-sandbox".to_string(),
            "--disable-dev-shm-usage".to_string(),
            "--disable-gpu".to_string(),
            "--window-size=1920,1080".to_string(),
        ])
        .build()
        .expect("Failed to build WebDriver configuration");

    // Configure browser connection pool for efficient resource management
    let pool_config = PoolConfigBuilder::default()
        .min_idle(1)
        .max_size(3)
        .connection_timeout(Duration::from_secs(30))
        .idle_timeout(Some(Duration::from_secs(300)))
        .build()
        .expect("Failed to build pool configuration");

    // Create and initialize the browser backend
    info!("Initializing browser backend");
    let backend = match BrowserBackend::builder()
        .with_config(webdriver_config)
        .with_pool_config(pool_config)
        .build()
    {
        Ok(backend) => {
            info!("Browser backend initialized successfully");
            backend
        }
        Err(e) => {
            error!("Failed to initialize browser backend: {}", e);
            error!("Ensure Chrome is installed and accessible");
            error!("Try running without --headless flag for debugging");
            return Err(Error::new(ErrorKind::Backend, e.to_string()));
        }
    };

    // Set up routing for different page types
    let router = Router::new()
        .route(Tag::new("dynamic_page"), scrape_dynamic_page)
        .route(Tag::new("form_page"), scrape_form_page)
        .route(Tag::new("spa_page"), scrape_spa_page)
        .route(Tag::new("error"), handle_browser_error);

    // Build the client with browser backend and routing
    let client = Client::new(backend, router)
        .with_request_queue(InMemDataset::stack())
        .with_dataset(InMemDataset::<String>::new());

    // Queue diverse page types for comprehensive testing
    info!("Queueing pages for browser automation");

    let queue = client.request_queue();

    // Standard HTML page with potential dynamic elements
    queue
        .push(Tag::new("dynamic_page"), "https://httpbin.org/html")
        .await?;

    // Form page for interaction testing
    queue
        .push(Tag::new("form_page"), "https://httpbin.org/forms/post")
        .await?;

    // Delayed content to simulate SPA loading
    queue
        .push(Tag::new("spa_page"), "https://httpbin.org/delay/2")
        .await?;

    info!("Starting browser automation process");

    // Execute the automation workflow
    match client.run().await {
        Ok(_) => {
            info!("Browser automation completed successfully");
        }
        Err(e) => {
            error!("Browser automation failed: {}", e);
            error!("This may indicate Chrome WebDriver setup issues");
            error!("Verify Chrome installation and ChromeDriver compatibility");
            return Err(e);
        }
    }

    info!("Chrome WebDriver example completed");

    // Note: In production applications, you would typically access the dataset
    // to retrieve and process the scraped results:
    //
    // let results = client.dataset().get_all().await?;
    // for result in results {
    //     info!("Scraped: {}", result);
    // }

    Ok(())
}
