//! The built-in handlers, alone and combined in tuples.

#![cfg(all(feature = "html", feature = "macros"))]

use std::future::{Future, ready};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use spire::backend::{Backend, Client};
use spire::dataset::InMemDataset;
use spire::extract::Select;
use spire::frontier::FrontierExt;
use spire::frontier::memory::InMemFrontier;
use spire::frontier::seed::Seed;
use spire::request::{Request, Response};
use spire::{Collect, Crawler, FollowLinks, Result, Router};

#[derive(Debug, Select)]
#[select("article")]
struct Product {
    #[select("h2")]
    name: String,
}

/// A shop with a listing paginated over three pages, counting fetches.
#[derive(Clone, Default)]
struct Shop {
    fetches: Arc<AtomicUsize>,
}

impl Backend for Shop {
    type Client = Visit;

    fn connect(&self) -> impl Future<Output = Result<Visit>> + Send {
        ready(Ok(Visit {
            shop: self.clone(),
            body: Mutex::new(""),
        }))
    }
}

struct Visit {
    shop: Shop,
    body: Mutex<&'static str>,
}

impl Client for Visit {
    fn fetch(&self, request: &Request) -> impl Future<Output = Result<Response<()>>> + Send {
        self.shop.fetches.fetch_add(1, Ordering::SeqCst);
        *self.body.lock().unwrap() = match request.uri().path() {
            "/list/1" => {
                r#"<article><h2>Lamp</h2></article><article><h2>Desk</h2></article>
                <a href="/about">about</a><a class="next" href="/list/2">next</a>"#
            }
            "/list/2" => {
                r#"<article><h2>Chair</h2></article>
                <a class="next" href="/list/3">next</a>"#
            }
            "/list/3" => r"<article><h2>Shelf</h2></article>",
            _ => "<p>other</p>",
        };
        ready(Ok(Response::new(())))
    }

    fn body(&self) -> impl Future<Output = Result<Bytes>> + Send {
        ready(Ok(Bytes::from_static(self.body.lock().unwrap().as_bytes())))
    }
}

#[tokio::test]
async fn collects_and_follows_pagination_with_one_fetch_per_page() {
    let router = Router::new().route(
        "listing",
        (
            Collect::<Product>::new(),
            FollowLinks::new("listing").select("a.next"),
        ),
    );
    let shop = Shop::default();
    let crawler = Crawler::new(shop.clone(), router)
        .with_frontier(InMemFrontier::fifo().dedup())
        .with_dataset(InMemDataset::<Product>::new())
        .with_seed(
            Seed::get("https://shop.test/list/1")
                .unwrap()
                .with_tag("listing"),
        );

    let stats = crawler.run().await.unwrap();
    assert_eq!((stats.requests, stats.failed), (3, 0), "{stats:?}");
    // Both handlers read every page, which is fetched once; `/about` is not
    // a pagination link, so it is not followed.
    assert_eq!(shop.fetches.load(Ordering::SeqCst), 3);

    let products = crawler
        .dataset::<Product>()
        .unwrap()
        .read_all()
        .await
        .unwrap();
    let names: Vec<_> = products.into_iter().map(|product| product.name).collect();
    assert_eq!(names, ["Lamp", "Desk", "Chair", "Shelf"]);
}

#[tokio::test]
async fn follows_every_link_by_default() {
    let router = Router::new()
        .route("listing", FollowLinks::new("page"))
        .fallback(|| async {});
    let shop = Shop::default();
    let crawler = Crawler::new(shop.clone(), router)
        .with_frontier(InMemFrontier::fifo().dedup())
        .with_seed(
            Seed::get("https://shop.test/list/1")
                .unwrap()
                .with_tag("listing"),
        );

    // The listing, then `/about` and `/list/2`, which the fallback handles.
    let stats = crawler.run().await.unwrap();
    assert_eq!((stats.requests, stats.failed), (3, 0), "{stats:?}");
}

#[test]
#[should_panic(expected = "invalid CSS selector `a[`")]
fn invalid_selectors_panic() {
    let _ = FollowLinks::new("page").select("a[");
}
