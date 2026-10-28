//! JSON APIs: typed responses and following pagination.
//!
//! httpbin's `/json` returns a sample slideshow, deserialized straight into
//! [`Slideshow`] with the `Json` extractor and stored slide by slide. httpbin's
//! `/get?page=N` echoes its query, standing in for a paginated API: each page
//! queues the next until [`LAST_PAGE`].

use serde::Deserialize;
use spire::backend::ReqwestBackend;
use spire::crawler::Tags;
use spire::dataset::InMemDataset;
use spire::extract::{Data, Json};
use spire::frontier::queue::{Link, Queue};
use spire::frontier::seed::Seed;
use spire::frontier::{FrontierExt, InMemFrontier};
use spire::{Crawler, Error, Result, Router};
use tracing_subscriber::EnvFilter;

/// The last page of the paginated endpoint.
const LAST_PAGE: u32 = 3;

/// The kinds of endpoint the crawl visits.
#[derive(Debug, Clone, Copy, Tags)]
enum Endpoint {
    Slideshow,
    Pages,
}

/// The body of httpbin's `/json`.
#[derive(Debug, Deserialize)]
struct Document {
    slideshow: Slideshow,
}

#[derive(Debug, Deserialize)]
struct Slideshow {
    title: String,
    author: String,
    slides: Vec<Slide>,
}

#[derive(Debug, Deserialize)]
struct Slide {
    title: String,
    #[serde(default)]
    items: Vec<String>,
}

/// The body of httpbin's `/get`: the query it was called with.
#[derive(Debug, Deserialize)]
struct Echo {
    args: Args,
    url: String,
}

#[derive(Debug, Deserialize)]
struct Args {
    page: String,
}

/// A page of the paginated endpoint.
#[derive(Debug)]
struct Visit {
    page: u32,
    url: String,
}

/// Stores every slide of the slideshow.
async fn slideshow(data: Data<Slide>, Json(document): Json<Document>) -> Result<()> {
    let Slideshow {
        title,
        author,
        slides,
    } = document.slideshow;
    tracing::info!("`{title}` by {author}: {} slides", slides.len());
    data.write_all(slides).await
}

/// Records the page and queues the next one.
async fn pages(queue: Queue, data: Data<Visit>, Json(echo): Json<Echo>) -> Result<()> {
    let page: u32 = echo
        .args
        .page
        .parse()
        .map_err(|error| Error::worker("page is not a number").with_source(error))?;
    data.write(Visit {
        page,
        url: echo.url,
    })
    .await?;

    if page < LAST_PAGE {
        let next = Link::from(format!("?page={}", page + 1)).with_tag(Endpoint::Pages);
        queue.follow(next).await?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // `RUST_LOG` picks the level, e.g. `RUST_LOG=debug`; `info` by default.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let router = Router::new()
        .route(Endpoint::Slideshow, slideshow)
        .route(Endpoint::Pages, pages)
        .require::<Endpoint>();

    let crawler = Crawler::new(ReqwestBackend::default(), router)
        .with_frontier(InMemFrontier::fifo().dedup().retry(2))
        .with_dataset(InMemDataset::<Slide>::new())
        .with_dataset(InMemDataset::<Visit>::new())
        .with_seed(Seed::get("https://httpbin.org/json")?.with_tag(Endpoint::Slideshow))
        .with_seed(Seed::get("https://httpbin.org/get?page=1")?.with_tag(Endpoint::Pages));

    crawler.run().await?;

    for slide in crawler.dataset::<Slide>()?.read_all().await? {
        tracing::info!("slide `{}`: {} items", slide.title, slide.items.len());
    }
    let mut visits = crawler.dataset::<Visit>()?.read_all().await?;
    visits.sort_by_key(|visit| visit.page);
    for visit in visits {
        tracing::info!("page {}: {}", visit.page, visit.url);
    }

    Ok(())
}
