//! [`HttpClient`] extractors.
//!
//! This module provides extractors specifically designed for HTTP-based web scraping
//! using the [`HttpClient`] backend. These extractors work with static HTML content
//! fetched via HTTP requests.
//!
//! # Available Extractors
//!
//! - [`Html`] - Parsed HTML document for direct DOM queries
//! - `Elements` - Declarative extraction of structured data from HTML
//!
//! # Examples
//!
//! ```ignore
//! use spire::extract::client::Html;
//! use scraper::Selector;
//!
//! async fn handler(Html(html): Html) {
//!     let selector = Selector::parse("h1").unwrap();
//!     for element in html.select(&selector) {
//!         println!("Title: {}", element.text().collect::<String>());
//!     }
//! }
//! ```
//!
//! [`HttpClient`]: spire_reqwest::HttpClient

mod html;

pub use html::Html;
