//! Basic usage: routing, link following and typed HTML extraction.
//!
//! The crawl starts from two seeds. httpbin's `/html` page is parsed into a
//! [`Story`] with `#[derive(Select)]`. httpbin's `/links/5/0` page links to
//! four others like it; their links are followed without a tag, so the path
//! route for `/links/**` handles them, and the deduplicating frontier visits
//! each page once.

use spire::backend::ReqwestBackend;
use spire::crawler::Tags;
use spire::dataset::{Data, InMemDataset};
use spire::extract::html::{Element, Links, Select};
use spire::frontier::queue::Queue;
use spire::frontier::seed::Seed;
use spire::frontier::{FrontierExt, InMemFrontier};
use spire::http::http::Uri;
use spire::{Crawler, Result, Router};
use tracing_subscriber::EnvFilter;

/// The kinds of page the crawl is seeded with.
#[derive(Debug, Clone, Copy, Tags)]
enum Page {
    Story,
    Links,
}

/// The story on httpbin's `/html` page.
#[derive(Debug, Select)]
struct Story {
    #[select("h1")]
    title: String,
    #[select("p")]
    paragraphs: Vec<String>,
}

/// A page of links, and how many it has.
#[derive(Debug)]
struct LinkPage {
    path: String,
    links: usize,
}

/// Stores the story.
async fn story(data: Data<Story>, Element(story): Element<Story>) -> Result<()> {
    data.write(story).await
}

/// Follows every link on the page; the followed pages come back here through
/// the path route.
async fn links(uri: Uri, queue: Queue, data: Data<LinkPage>, links: Links) -> Result<()> {
    data.write(LinkPage {
        path: uri.path().to_owned(),
        links: links.len(),
    })
    .await?;
    queue.follow_all(links).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // `RUST_LOG` picks the level, e.g. `RUST_LOG=debug`; `info` by default.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let router = Router::new()
        .route(Page::Story, story)
        .route(Page::Links, links)
        .route_path("/links/**", links)
        .require::<Page>();

    let crawler = Crawler::new(ReqwestBackend::default(), router)
        .with_frontier(InMemFrontier::fifo().dedup().max_depth(2).retry(2))
        .with_dataset(InMemDataset::<Story>::new())
        .with_dataset(InMemDataset::<LinkPage>::new())
        .with_seed(Seed::get("https://httpbin.org/html")?.with_tag(Page::Story))
        .with_seed(Seed::get("https://httpbin.org/links/5/0")?.with_tag(Page::Links));

    crawler.run().await?;

    for story in crawler.dataset::<Story>()?.read_all().await? {
        let words: usize = story
            .paragraphs
            .iter()
            .map(|paragraph| paragraph.split_whitespace().count())
            .sum();
        tracing::info!("story `{}`: {words} words", story.title);
    }
    for page in crawler.dataset::<LinkPage>()?.read_all().await? {
        tracing::info!("{}: {} links", page.path, page.links);
    }

    Ok(())
}
