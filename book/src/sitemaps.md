# Sitemaps

A sitemap lists a site's pages, often more of them than its links lead to.
With the `sitemap` feature, the `FollowSitemap` handler follows sitemaps, and
the `Sitemap` extractor parses one for a handler of your own.

## Following sitemaps

`FollowSitemap::new(tag)` queues every page of a sitemap with `tag`. Route the
sitemap's own tag to it, then either seed sitemaps yourself, or let
`Robots::with_sitemaps` queue the ones each site's robots.txt lists, the first
time it is fetched:

```rust,no_run
use spire::backend::ReqwestBackend;
use spire::crawler::{Robots, Tags};
use spire::frontier::seed::Seed;
use spire::frontier::{FrontierExt, InMemFrontier};
use spire::handler::FollowSitemap;
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

    let crawler = Crawler::new(ReqwestBackend::default(), router)
        .with_frontier(InMemFrontier::fifo().dedup().allow_hosts(["example.com"]))
        // The sitemaps of each site's robots.txt, tagged to be followed.
        .with_robots(Robots::new("MyBot").with_sitemaps(Page::Sitemap))
        .with_seed(Seed::get("https://example.com/")?.with_tag(Page::Home))
        // A sitemap robots.txt does not list.
        .with_seed(Seed::get("https://example.com/products.xml")?.with_tag(Page::Sitemap));
    crawler.run().await?;
    Ok(())
}
```

Sitemaps go through the frontier like any other seed, so its scope,
deduplication and depth limits apply to them and to their pages.

## Formats and indexes

A sitemap is an XML `urlset`, an XML sitemap index, or a text file with one
URL per line, gzipped or not. It is parsed off the async threads, and capped
by the protocol's limits of 50,000 URLs and 50 MiB: the records before a limit
are kept, and a warning is logged. Fetch sitemaps over HTTP, as a browser
renders XML as a page.

An index lists more sitemaps. `FollowSitemap` queues them with the sitemap's
own tag, so they are followed in turn, through indexes of any depth. A
sitemap's pages are one level deeper than the sitemap, so a depth limit
bounds how deep indexes are followed too.

## Priorities

A page's `<priority>`, from 0.0 to 1.0, ranks it against the other pages of
its site, not against the rest of the crawl. So `FollowSitemap` queues a
sitemap's pages highest priority first, all at the default seed priority.
This orders pages within each sitemap, not across the site: a FIFO frontier
keeps their insertion order, while a LIFO frontier pops them in reverse.
Pages from different sitemaps are not ranked together.

## Your own handler

For anything else, such as keeping only some pages, or mapping priorities
onto the crawl's own scale, take the `Sitemap` extractor. Its records are
pages, with their `<lastmod>`, `<changefreq>`, `<priority>` and image, video,
news and alternate-language extensions, or the sitemaps of an index:

```rust,no_run
use spire::crawler::Tags;
use spire::extract::sitemap::{Record, Sitemap};
use spire::frontier::queue::Queue;
use spire::frontier::seed::Seed;
use spire::Result;

#[derive(Debug, Clone, Copy, Tags)]
enum Page {
    Sitemap,
    Product,
}

async fn sitemap(queue: Queue, Sitemap(records): Sitemap) -> Result<()> {
    for record in records {
        match record {
            // Only products, ranked by their priority among the crawl's seeds.
            Record::Entry(page) if page.location.path().starts_with("/products/") => {
                let priority = (page.priority.unwrap_or_default().get() * 10.0) as i32;
                let seed = Seed::get(page.location.as_str())?
                    .with_tag(Page::Product)
                    .with_priority(priority);
                queue.follow(seed).await?;
            }
            Record::Entry(_) => {}
            // The sitemaps of an index, followed by this handler again.
            Record::Sitemap(index) => {
                let seed = Seed::get(index.location.as_str())?.with_tag(Page::Sitemap);
                queue.follow(seed).await?;
            }
        }
    }
    Ok(())
}
```

A priority frontier then crawls the products by their priority; see
[Seeds and the frontier](frontier.md).
