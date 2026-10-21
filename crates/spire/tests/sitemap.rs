//! A crawl seeded only through robots.txt and sitemaps.

#![cfg(all(feature = "sitemap", feature = "robots", feature = "macros"))]

use std::future::{Future, ready};
use std::io::Write;
use std::sync::Mutex;

use bytes::Bytes;
use flate2::Compression;
use flate2::write::GzEncoder;
use spire::backend::{Backend, Client};
use spire::dataset::{Data, InMemDataset};
use spire::frontier::FrontierExt;
use spire::frontier::memory::InMemFrontier;
use spire::frontier::seed::Seed;
use spire::request::http::{StatusCode, Uri};
use spire::request::{Request, Response, Tags};
use spire::{Crawler, FollowSitemap, Result, Robots, Router};

#[derive(Debug, Clone, Copy, Tags)]
enum Page {
    Home,
    Sitemap,
    Product,
}

/// A site whose robots.txt points to a sitemap index of a plain and a
/// gzipped sitemap.
#[derive(Clone)]
struct Site;

fn gzip(text: &str) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(text.as_bytes()).unwrap();
    encoder.finish().unwrap()
}

impl Backend for Site {
    type Client = Visit;

    fn connect(&self) -> impl Future<Output = Result<Visit>> + Send {
        ready(Ok(Visit(Mutex::new(Bytes::new()))))
    }
}

/// A request to [`Site`], keeping the body it was served.
struct Visit(Mutex<Bytes>);

impl Client for Visit {
    fn fetch(&self, request: &Request) -> impl Future<Output = Result<Response<()>>> + Send {
        let body: Vec<u8> = match request.uri().path() {
            "/robots.txt" => {
                b"User-agent: *\nDisallow: /private\nSitemap: https://shop.test/sitemap.xml\n"
                    .to_vec()
            }
            "/sitemap.xml" => {
                br#"<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                <sitemap><loc>https://shop.test/products.xml</loc></sitemap>
                <sitemap><loc>https://shop.test/archive.xml.gz</loc></sitemap>
            </sitemapindex>"#
                    .to_vec()
            }
            "/products.xml" => br#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                <url><loc>https://shop.test/products/1</loc></url>
                <url><loc>https://shop.test/private/2</loc></url>
                <url><loc>https://elsewhere.test/products/3</loc></url>
            </urlset>"#
                .to_vec(),
            "/archive.xml.gz" => {
                gzip("https://shop.test/products/4\nhttps://shop.test/products/1\n")
            }
            "/" | "/products/1" | "/products/4" => b"<html></html>".to_vec(),
            _ => {
                let mut head = Response::new(());
                *head.status_mut() = StatusCode::NOT_FOUND;
                return ready(Ok(head));
            }
        };
        *self.0.lock().unwrap() = Bytes::from(body);
        ready(Ok(Response::new(())))
    }

    fn body(&self) -> impl Future<Output = Result<Bytes>> + Send {
        ready(Ok(self.0.lock().unwrap().clone()))
    }
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
    let crawler = Crawler::new(Site, router)
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
    let crawler = Crawler::new(Site, router)
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
