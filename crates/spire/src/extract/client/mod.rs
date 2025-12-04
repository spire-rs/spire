//! [`HttpClient`] extractors.
//!
//! This module provides extractors specifically designed for HTTP-based web scraping
//! using the [`HttpClient`] backend. These extractors work with static HTML content
//! fetched via HTTP requests.
//!
//! # Available Extractors
//!
//! - [`HttpBackend`] - Direct access to the underlying HTTP client (reqwest feature)
//! - `Elements` - Declarative extraction of structured data from HTML
//!
//! [`HttpClient`]: spire_reqwest::HttpClient

#[cfg(feature = "reqwest")]
mod http_client;
#[cfg(feature = "reqwest")]
#[cfg_attr(docsrs, doc(cfg(feature = "reqwest")))]
pub use http_client::HttpBackend;
