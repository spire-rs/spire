//! What the crawl does with the pages: the handlers, their shared state, and
//! the stored posts.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::Serialize;
use spire::Result;
use spire::crawler::Depth;
use spire::dataset::Data;
use spire::extract::html::Element;
use spire::extract::{FromRef, State};
use spire::http::http::Uri;

use crate::pages::{Article, Page};

/// A post as stored in `rust-blog.jsonl`.
#[derive(Debug, Serialize)]
pub struct Post {
    pub url: String,
    pub title: String,
    pub date: String,
    pub authors: String,
    pub words: usize,
    pub quotes: bool,
}

/// State shared by every handler: the words read so far.
#[derive(Clone, Default)]
pub struct Blog {
    pub words: Arc<AtomicUsize>,
}

/// The part of the state a handler asks for with `State<Words>`.
pub struct Words(Arc<AtomicUsize>);

impl FromRef<Blog> for Words {
    fn from_ref(blog: &Blog) -> Self {
        Self(blog.words.clone())
    }
}

/// Logs every page: an untagged home page, or a tagged sitemap or post.
pub async fn visit(page: Option<Page>, uri: Uri, depth: Depth) {
    tracing::info!(?page, %uri, depth = depth.get(), "visiting");
}

/// Stores a post.
pub async fn post(
    uri: Uri,
    State(Words(words)): State<Words>,
    posts: Data<Post>,
    Element(article): Element<Article>,
) -> Result<()> {
    let (date, authors) = article
        .byline
        .split_once('·')
        .unwrap_or((&article.byline, ""));
    let count = article
        .paragraphs
        .iter()
        .map(|paragraph| paragraph.split_whitespace().count())
        .sum();
    words.fetch_add(count, Ordering::Relaxed);

    posts
        .write(Post {
            url: uri.to_string(),
            title: article.title,
            date: date.trim().to_owned(),
            authors: authors.trim().to_owned(),
            words: count,
            quotes: article.quote.is_some(),
        })
        .await
}
