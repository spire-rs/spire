//! What the crawl visits and extracts.

use spire::extract::html::Select;
use spire::request::Tags;

/// The kinds of page the crawl visits, routed by tag. `Router::require`
/// checks that each has a route.
#[derive(Debug, Clone, Copy, Tags)]
pub enum Page {
    Sitemap,
    Post,
}

/// A post, as its page shows it.
#[derive(Debug, Select)]
pub struct Article {
    #[select("header h2")]
    pub title: String,
    /// `May 15, 2015 · The Rust Core Team`.
    #[select(".publish-date-author")]
    pub byline: String,
    /// Only on some posts.
    #[select(".post blockquote")]
    pub quote: Option<String>,
    #[select(".post p")]
    pub paragraphs: Vec<String>,
}

/// A link a post cites, collected by `Collect`: one per match of the root
/// selector.
#[derive(Debug, Select)]
#[select(".post a[rel=external]")]
pub struct Citation {
    #[select(attr = "href")]
    pub href: String,
}
