# Sitemaps

With the `sitemap` feature, the `FollowSitemap` handler follows sitemaps:
every page is queued with one tag, and every sitemap of an index with the tag
of the sitemap itself. Seed sitemaps yourself, or let `Robots::with_sitemaps`
queue the ones each site's robots.txt lists:

```rust,no_run
use spire::backend::HttpClient;
use spire::crawler::Robots;
use spire::frontier::seed::Seed;
use spire::frontier::{FrontierExt, InMemFrontier};
use spire::handler::FollowSitemap;
use spire::request::Tags;
use spire::{Crawler, Result, Router};

#[derive(Debug, Clone, Copy, Tags)]
enum Page {
    Home,
    Sitemap,
    Product,
}

async fn product() {}

async fn crawl() -> Result<()> {
    let router = Router::new()
        .route(Page::Home, || async {})
        .route(Page::Sitemap, FollowSitemap::new(Page::Product))
        .route(Page::Product, product);

    let crawler = Crawler::new(HttpClient::default(), router)
        .with_frontier(InMemFrontier::fifo().dedup().allow_hosts(["example.com"]))
        .with_robots(Robots::new("MyBot").with_sitemaps(Page::Sitemap))
        .with_seed(Seed::get("https://example.com/")?.with_tag(Page::Home));
    crawler.run().await?;
    Ok(())
}
```

The `Sitemap` extractor parses a sitemap for a handler of your own. Sitemaps
must be fetched over HTTP: a browser renders XML as a page.
