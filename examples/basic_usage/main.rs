//! Basic usage example demonstrating core Spire functionality.
//!
//! This example shows fundamental concepts of the Spire framework. It demonstrates
//! how to set up an HTTP scraping client with the reqwest backend and define handlers
//! for different content types using extractors. The example covers managing request
//! queues with tag-based routing, extracting and processing text and HTML data from
//! responses, and storing scraped data using custom dataset types.

mod data;

use data::PageContent;
use spire::dataset::{Data, InMemDataset};
use spire::extract::{Html, Text};
use spire::frontier::{FrontierExt, InMemFrontier, Seed};
use spire::{Crawler, HttpClient, Result, Router, http};

/// Handler for processing HTML pages.
///
/// This handler demonstrates how to extract HTML content from responses and
/// store basic page data in custom datasets.
async fn scrape_html_page(uri: http::Uri, data_store: Data<PageContent>, page: Html) -> Result<()> {
    let url = uri.to_string();
    tracing::info!("Processing HTML page: {}", url);

    let content_length = page.html().len();
    let title = page.title().unwrap_or_else(|| "HTML Page".to_string());
    let page_content =
        PageContent::new(url, "text/html".to_string(), content_length).with_title(title);

    data_store.write(page_content).await?;
    Ok(())
}

/// Handler for processing plain text content.
///
/// This handler demonstrates basic text analysis and metrics collection for plain
/// text responses from web servers.
async fn scrape_text_content(
    uri: http::Uri,
    data_store: Data<PageContent>,
    Text(content): Text,
) -> Result<()> {
    let url = uri.to_string();
    tracing::info!("Processing text content: {}", url);

    let content_length = content.len();
    let mut metadata = Vec::new();

    // Analyze text content
    let line_count = content.lines().count();
    let word_count = content.split_whitespace().count();
    let char_count = content.chars().count();

    metadata.push(format!("Lines: {line_count}"));
    metadata.push(format!("Words: {word_count}"));
    metadata.push(format!("Characters: {char_count}"));

    // Check if it looks like robots.txt
    if content.to_lowercase().contains("user-agent") {
        metadata.push("Content type: robots.txt".to_string());
    }

    let page_content = PageContent::new(url, "text/plain".to_string(), content_length)
        .with_title("Text Content".to_string())
        .with_metadata(metadata);

    data_store.write(page_content).await?;
    Ok(())
}

/// Creates and configures the crawler with its initial requests.
fn create_crawler() -> Result<Crawler<HttpClient>> {
    let router = Router::new()
        .route("text_content", scrape_text_content)
        .route("html_page", scrape_html_page);

    let crawler = Crawler::new(HttpClient::default(), router)
        .with_frontier(InMemFrontier::fifo().dedup().retry(2))
        .with_dataset(InMemDataset::<PageContent>::new())
        .with_seed(Seed::get("https://httpbin.org/html")?.with_tag("html_page"))
        .with_seed(Seed::get("https://httpbin.org/robots.txt")?.with_tag("text_content"));

    Ok(crawler)
}

/// Displays the scraping results
async fn display_results(dataset: &Data<PageContent>) -> Result<()> {
    let results = dataset.read_all().await?;

    tracing::info!("Scraping completed! Processed {} items:", results.len());

    for (i, page_content) in results.iter().enumerate() {
        tracing::info!(
            "{}: {} - {}",
            i + 1,
            page_content.title.as_deref().unwrap_or("No Title"),
            page_content.url
        );
        tracing::info!(
            "   Content Type: {}, Size: {} bytes",
            page_content.content_type,
            page_content.content_length
        );
        tracing::info!("   Scraped at: {}", page_content.scraped_at);

        if !page_content.metadata.is_empty() {
            tracing::info!("   Metadata:");
            for metadata_item in &page_content.metadata {
                tracing::info!("     - {}", metadata_item);
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize structured logging
    tracing_subscriber::fmt().init();

    tracing::info!("Starting Spire Basic Usage Example");

    let crawler = create_crawler()?;

    tracing::info!("Starting scraping process");
    let stats = crawler.run().await?;
    tracing::info!(?stats, "Scraping process finished");

    // Display results
    let dataset = crawler.dataset::<PageContent>()?;
    display_results(&dataset).await?;

    tracing::info!("Basic usage example completed successfully");
    Ok(())
}
