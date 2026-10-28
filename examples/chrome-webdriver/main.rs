//! Browser automation: rendering pages in Chrome over WebDriver.
//!
//! `ThirtyfourTab` navigates the browser and hands the handler the request's tab;
//! the page is parsed with `tab.snapshot()` once the handler is done
//! interacting, so it includes whatever scripts rendered. Links are followed as with any other
//! backend.
//!
//! Needs a WebDriver server on port 4444 matching the installed Chrome, e.g.
//! from Chrome for Testing:
//!
//! ```bash
//! npx @puppeteer/browsers install chromedriver@stable
//! ./chromedriver/<platform>/chromedriver-<platform>/chromedriver --port=4444
//! ```

use spire::backend::ThirtyfourBackend;
use spire::crawler::Tags;
use spire::dataset::InMemDataset;
use spire::extract::{Data, ThirtyfourTab};
use spire::frontier::queue::{Link, Queue};
use spire::frontier::seed::Seed;
use spire::frontier::{FrontierExt, InMemFrontier};
use spire::{Crawler, Error, Result, Router};
use tracing_subscriber::EnvFilter;

/// The kinds of page the crawl visits.
#[derive(Debug, Clone, Copy, Tags)]
enum Page {
    Rendered,
}

/// A page as the browser rendered it.
#[derive(Debug)]
struct Rendered {
    url: String,
    title: String,
    links: usize,
}

/// Reads the rendered page and follows its links.
async fn rendered(queue: Queue, data: Data<Rendered>, tab: ThirtyfourTab) -> Result<()> {
    let title = tab
        .title()
        .await
        .map_err(|error| Error::backend("failed to read the title").with_source(error))?;
    let url = tab
        .current_url()
        .await
        .map_err(|error| Error::backend("failed to read the url").with_source(error))?;

    let page = tab.snapshot().await?;
    let links = page.links(&url.as_str().parse()?);
    data.write(Rendered {
        url: url.to_string(),
        title,
        links: links.len(),
    })
    .await?;

    let links = links
        .into_iter()
        .map(|link| Link::from(link).with_tag(Page::Rendered));
    queue.follow_all(links).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // `RUST_LOG` picks the level, e.g. `RUST_LOG=debug`; `info` by default.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let backend = ThirtyfourBackend::builder()
        .remote("http://127.0.0.1:4444")
        .max_sessions(2)
        .build()?;

    let router = Router::new()
        .route(Page::Rendered, rendered)
        .require::<Page>();

    let crawler = Crawler::new(backend, router)
        .with_frontier(InMemFrontier::fifo().dedup().max_depth(1))
        .with_dataset(InMemDataset::<Rendered>::new())
        .with_seed(Seed::get("https://httpbin.org/links/3/0")?.with_tag(Page::Rendered));

    let outcome = crawler.run().await;
    // Quit the browser sessions instead of dropping them.
    crawler.backend().close().await;
    outcome?;

    for page in crawler.dataset::<Rendered>()?.read_all().await? {
        tracing::info!("{} `{}`: {} links", page.url, page.title, page.links);
    }

    Ok(())
}
