//! [`HttpClient`] extractors.
//!
//! This module provides extractors specifically designed for HTTP-based web scraping
//! using the [`HttpClient`] backend. These extractors work with static HTML content
//! fetched via HTTP requests.
//!
//! # Available Extractors
//!
//! - [`Html`] - Parsed HTML document for direct DOM queries
//! - [`Elements`] - Declarative extraction of structured data from HTML
//!
//! [`HttpClient`]: spire_reqwest::HttpClient

use std::ops::{Deref, DerefMut};

use scraper::Html as HtmlDoc;

use crate::backend::Client;
use crate::context::Context;
use crate::extract::{Elements, FromContext, Select, Text};
use crate::Error;

/// Parsed HTML document extractor.
///
/// Extracts and parses the response body as HTML, providing access to the DOM
/// structure for querying and data extraction. Built on top of the [`scraper`] crate.
///
/// # Examples
///
/// ```ignore
/// use spire::extract::client::Html;
/// use scraper::Selector;
///
/// async fn handler(Html(html): Html) {
///     let selector = Selector::parse("h1").unwrap();
///     for element in html.select(&selector) {
///         println!("Title: {}", element.text().collect::<String>());
///     }
/// }
/// ```
///
/// [`scraper`]: https://docs.rs/scraper
#[derive(Debug, Clone)]
pub struct Html(pub HtmlDoc);

#[async_trait::async_trait]
impl<C, S> FromContext<C, S> for Html
where
    C: Client,
    S: Sync,
{
    type Rejection = Error;

    async fn from_context(cx: Context<C>, state: &S) -> Result<Self, Self::Rejection> {
        let Text(text) = Text::from_context(cx, state).await?;
        let html = HtmlDoc::parse_document(&text);
        Ok(Self(html))
    }
}

impl Deref for Html {
    type Target = HtmlDoc;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Html {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(all(feature = "macros", feature = "reqwest"))]
#[async_trait::async_trait]
impl<S, T> FromContext<spire_reqwest::HttpClient, S> for Elements<T>
where
    S: Sync + Send + 'static,
    T: Select + Send,
{
    type Rejection = Error;

    async fn from_context(
        cx: Context<spire_reqwest::HttpClient>,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let Html(_html) = Html::from_context(cx, state).await?;
        todo!("Elements extractor for HttpClient not yet implemented")
    }
}
