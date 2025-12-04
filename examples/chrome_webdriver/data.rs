//! Data structures for the chrome_webdriver example.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// Structure representing page data extracted through browser automation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageData {
    pub url: String,
    pub title: String,
    pub content_length: usize,
    pub processing_method: String,
    pub processed_at: Timestamp,
}

impl PageData {
    /// Creates a new PageData instance.
    pub fn new(
        url: String,
        title: String,
        content_length: usize,
        processing_method: String,
    ) -> Self {
        Self {
            url,
            title,
            content_length,
            processing_method,
            processed_at: Timestamp::now(),
        }
    }
}
