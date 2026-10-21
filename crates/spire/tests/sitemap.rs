//! A crawl seeded only through robots.txt and sitemaps.

#![cfg(all(
    feature = "sitemap",
    feature = "robots",
    feature = "macros",
    feature = "testing"
))]

use std::io::Write;

use flate2::Compression;
use flate2::write::GzEncoder;
use spire::dataset::{Data, InMemDataset};
use spire::frontier::FrontierExt;
use spire::frontier::memory::InMemFrontier;
use spire::frontier::seed::Seed;
use spire::request::Tags;
use spire::request::http::Uri;
use spire::testing::{MockPage, MockSite};
use spire::{Crawler, FollowSitemap, Result, Robots, Router};

#[derive(Debug, Clone, Copy, Tags)]
enum Page {
    Home,
    Sitemap,
    Product,
}

fn gzip(text: &str) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(text.as_bytes()).unwrap();
    encoder.finish().unwrap()
}

/// A site whose robots.txt points to a sitemap index of a plain and a
/// gzipped sitemap.
fn site() -> MockSite {
    let robots = "User-agent: *\nDisallow: /private\nSitemap: https://shop.test/sitemap.xml\n";
    let index = r#"<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
        <sitemap><loc>https://shop.test/products.xml</loc></sitemap>
        <sitemap><loc>https://shop.test/archive.xml.gz</loc></sitemap>
    </sitemapindex>"#;
    let products = r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
        <url><loc>https://shop.test/products/1</loc></url>
        <url><loc>https://shop.test/private/2</loc></url>
        <url><loc>https://elsewhere.test/products/3</loc></url>
    </urlset>"#;
    let archive = gzip("https://shop.test/products/4\nhttps://shop.test/products/1\n");

    let mut site = MockSite::new()
        .page("https://shop.test/robots.txt", MockPage::text(robots))
        .page("https://shop.test/sitemap.xml", MockPage::xml(index))
        .page("https://shop.test/products.xml", MockPage::xml(products))
        .page("https://shop.test/archive.xml.gz", MockPage::text(archive));
    for page in ["/", "/products/1", "/products/4"] {
        site = site.page(
            &format!("https://shop.test{page}"),
            MockPage::html("<html></html>"),
        );
    }
    site
}

#[tokio::test]
async fn crawls_the_sitemaps_robots_txt_lists() {
    async fn product(uri: Uri, data: Data<String>) -> Result<()> {
        data.write(uri.path().to_owned()).await
    }

    let router = Router::new()
        .route(Page::Home, || async {})
        .route(Page::Sitemap, FollowSitemap::new(Page::Product))
        .route(Page::Product, product)
        .require::<Page>();
    let crawler = Crawler::new(site(), router)
        .with_frontier(InMemFrontier::fifo().dedup().allow_hosts(["shop.test"]))
        .with_robots(Robots::new("bot").with_sitemaps(Page::Sitemap))
        .with_dataset(InMemDataset::<String>::new())
        .with_seed(
            Seed::get("https://shop.test/")
                .unwrap()
                .with_tag(Page::Home),
        );

    // The home page, the index, both sitemaps and two products; the
    // disallowed `/private/2` is skipped.
    let stats = crawler.run().await.unwrap();
    let counts = (stats.requests, stats.failed, stats.skipped);
    assert_eq!(counts, (6, 0, 1), "{stats:?}");

    // `/private/2` is disallowed by robots.txt, `elsewhere.test` is out of
    // scope, and `/products/1`, listed twice, is crawled once.
    let mut products = crawler
        .dataset::<String>()
        .unwrap()
        .read_all()
        .await
        .unwrap();
    products.sort();
    assert_eq!(products, ["/products/1", "/products/4"]);
}

#[tokio::test]
async fn follows_untagged_sitemaps_by_path() {
    async fn product(uri: Uri, data: Data<String>) -> Result<()> {
        data.write(uri.path().to_owned()).await
    }

    // Nested sitemaps keep the index's lack of a tag, so the path routes
    // pick them up as well.
    let router = Router::new()
        .route_path("/*.xml", FollowSitemap::new(Page::Product))
        .route_path("/*.xml.gz", FollowSitemap::new(Page::Product))
        .route(Page::Product, product);
    let crawler = Crawler::new(site(), router)
        .with_frontier(InMemFrontier::fifo().dedup().allow_hosts(["shop.test"]))
        .with_dataset(InMemDataset::<String>::new())
        .with_seed(Seed::get("https://shop.test/sitemap.xml").unwrap());

    let stats = crawler.run().await.unwrap();
    assert_eq!(stats.failed, 0, "{stats:?}");

    let mut products = crawler
        .dataset::<String>()
        .unwrap()
        .read_all()
        .await
        .unwrap();
    products.sort();
    assert_eq!(products, ["/private/2", "/products/1", "/products/4"]);
}
