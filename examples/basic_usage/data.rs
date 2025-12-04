//! Data structures for storing scraped page content.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// Represents content scraped from a web page with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageContent {
    pub url: String,
    pub title: Option<String>,
    pub content_type: String,
    pub content_length: usize,
    pub scraped_at: Timestamp,
    pub metadata: Vec<String>,
}

impl PageContent {
    /// Creates a new PageContent instance with basic information.
    pub fn new(url: String, content_type: String, content_length: usize) -> Self {
        Self {
            url,
            title: None,
            content_type,
            content_length,
            scraped_at: Timestamp::now(),
            metadata: Vec::new(),
        }
    }

    /// Sets the title of the page content.
    pub fn with_title(mut self, title: String) -> Self {
        self.title = Some(title);
        self
    }

    /// Adds multiple metadata entries at once.
    pub fn with_metadata(mut self, metadata: Vec<String>) -> Self {
        self.metadata.extend(metadata);
        self
    }
}
