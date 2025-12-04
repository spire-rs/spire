//! Data structures for the API endpoints example.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Structure representing a processed user from API data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub source_url: String,
    pub processed_at: Timestamp,
}

impl User {
    /// Creates a new User from JSON data and source URL.
    pub fn from_json(data: Value, source_url: String) -> spire::Result<Self> {
        // Try to extract user-like information from various JSON structures
        let (name, email, id) = if let Some(obj) = data.as_object() {
            // Look for slideshow data (httpbin.org/json structure)
            if let Some(slideshow) = obj.get("slideshow") {
                if let Some(slideshow_obj) = slideshow.as_object() {
                    let title = slideshow_obj
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("API User");
                    let author = slideshow_obj
                        .get("author")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown Author");

                    (
                        format!("{} ({})", title, author),
                        format!(
                            "{}@api.example.com",
                            author.to_lowercase().replace(' ', ".")
                        ),
                        Self::generate_id_from_string(&title),
                    )
                } else {
                    ("API Data".to_string(), "api@example.com".to_string(), 1)
                }
            } else if let Some(uuid) = obj.get("uuid") {
                // Handle UUID responses
                let uuid_str = uuid.as_str().unwrap_or("unknown-uuid");
                (
                    format!("UUID User {}", &uuid_str[..8]),
                    format!("uuid-{}@api.example.com", &uuid_str[..8]),
                    Self::generate_id_from_string(uuid_str),
                )
            } else {
                // Generic object handling
                let name = obj.keys().next().map_or("object", |v| v).to_string();
                (
                    format!("API Object User ({})", name),
                    format!("{}@api.example.com", name),
                    Self::generate_id_from_string(&name),
                )
            }
        } else if let Some(uuid_str) = data.as_str() {
            // Direct UUID string
            (
                format!("String User {}", &uuid_str[..8.min(uuid_str.len())]),
                format!("string@api.example.com"),
                Self::generate_id_from_string(uuid_str),
            )
        } else {
            // Fallback for other data types
            (
                "Generic API User".to_string(),
                "generic@api.example.com".to_string(),
                Self::generate_id_from_string(&source_url),
            )
        };

        Ok(Self {
            id,
            name,
            email,
            source_url,
            processed_at: Timestamp::now(),
        })
    }

    /// Generates a deterministic ID from a string.
    fn generate_id_from_string(input: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        input.hash(&mut hasher);
        (hasher.finish() % 100000) + 1
    }
}

/// Structure representing API metadata and processing information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMetadata {
    pub endpoint: String,
    pub response_size: usize,
    pub fields_count: usize,
    pub processed_at: Timestamp,
}

impl ApiMetadata {
    /// Creates new API metadata.
    pub fn new(endpoint: String, response_size: usize, fields_count: usize) -> Self {
        Self {
            endpoint,
            response_size,
            fields_count,
            processed_at: Timestamp::now(),
        }
    }
}
