//! A crawl of the Rust blog, touring every feature of Spire.
//!
//! The crawl starts from the blog's home page. Its robots.txt lists the
//! sitemap, which lists every post, and the home page links to them too:
//! both routes lead to the same posts, and deduplication crawls each once.
//! Every post is stored in `rust-blog.jsonl`, and the links posts cite are
//! collected along the way.
//!
//! The frontier is kept in `rust-blog.redb`: stop the crawl with Ctrl-C and
//! run it again to resume where it stopped. Delete both files to start over.
//! `SPIRE_MAX_REQUESTS` bounds the requests of a run, 50 by default, and the
//! blog gets one request a second: a full crawl of its ~800 posts takes
//! about 15 minutes over several runs.
//!
//! After a crash, redb queues the requests that were in flight again, so no
//! post is missed, but posts handled in the last moments before it may be
//! appended to `rust-blog.jsonl` twice: dedupe by `url` when reading it.

mod handlers;
mod pages;
mod scope;

use std::sync::atomic::Ordering;
use std::time::Duration;

use spire::backend::HttpClient;
use spire::crawler::Robots;
use spire::dataset::{InMemDataset, JsonlDataset};
use spire::frontier::FrontierExt;
use spire::frontier::redb::RedbStore;
use spire::frontier::seed::Seed;
use spire::handler::{Collect, FollowLinks, FollowSitemap};
use spire::{Crawler, Result, Router};
use tracing_subscriber::EnvFilter;

use crate::handlers::{Blog, Post, post, visit};
use crate::pages::{Citation, Page};

#[tokio::main]
async fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let max_requests = std::env::var("SPIRE_MAX_REQUESTS")
        .ok()
        .and_then(|max| max.parse().ok())
        .unwrap_or(50);

    let blog = Blog::default();
    let router = Router::with_state(blog.clone())
        // The untagged home page is routed by path. A tuple runs both
        // handlers on the page, fetched once.
        .route_path("/", (visit, FollowLinks::new(Page::Post)))
        // Sitemaps found through robots.txt: their pages become posts.
        .route(Page::Sitemap, (visit, FollowSitemap::new(Page::Post)))
        // Three handlers on each post: log it, store it, collect its citations.
        .route(Page::Post, (visit, post, Collect::<Citation>::new()))
        .require::<Page>();

    // The frontier is stored in redb, so a stopped crawl resumes. Facades
    // add deduplication (over redb's seen set), scope, depth, retries and
    // per-tag backoff. `BloomSeen` dedups in fixed memory instead.
    let store = RedbStore::open("rust-blog.redb").await?;
    let frontier = store
        .frontier()
        .dedup_with(store.seen())
        .allow_hosts(["blog.rust-lang.org"])
        .filter(scope::in_scope)
        .max_depth(2)
        .retry(3)
        .tag_backoff();

    let crawler = Crawler::new(HttpClient::default(), router)
        .with_frontier(frontier)
        // robots.txt is obeyed, and the sitemaps it lists are queued.
        .with_robots(Robots::new("spire-example").with_sitemaps(Page::Sitemap))
        // Polite: one request at a time to the blog, a second apart.
        .with_concurrency(4)
        .with_host_concurrency(1)
        .with_host_delay(Duration::from_secs(1))
        .with_max_requests(max_requests)
        .with_request_timeout(Duration::from_secs(30))
        .with_shutdown_grace(Duration::from_secs(10))
        // Posts are appended to a JSON Lines file, citations kept in memory.
        .with_dataset(JsonlDataset::<Post>::append("rust-blog.jsonl").await?)
        .with_dataset(InMemDataset::<Citation>::new())
        .with_seed(Seed::get("https://blog.rust-lang.org/")?);

    // Ctrl-C stops the crawl gracefully: requests in flight finish, and the
    // rest stays queued in redb for the next run.
    let shutdown = crawler.shutdown_token();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            shutdown.cancel();
        }
    });

    let stats = crawler.run().await?;
    let citations = crawler.dataset::<Citation>()?.read_all().await?;
    let external = citations
        .iter()
        .filter(|citation| !citation.href.contains("rust-lang.org"))
        .count();

    tracing::info!(
        requests = stats.requests,
        failed = stats.failed,
        words = blog.words.load(Ordering::Relaxed),
        citations = citations.len(),
        external,
        "posts appended to rust-blog.jsonl",
    );
    Ok(())
}
