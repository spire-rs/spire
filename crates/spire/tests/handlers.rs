//! The built-in handlers, alone and combined in tuples.

#![cfg(all(feature = "html", feature = "macros", feature = "testing"))]

use spire::dataset::InMemDataset;
use spire::extract::Select;
use spire::frontier::FrontierExt;
use spire::frontier::memory::InMemFrontier;
use spire::frontier::seed::Seed;
use spire::testing::{MockPage, MockSite};
use spire::{Collect, Crawler, FollowLinks, Router};

#[derive(Debug, Select)]
#[select("article")]
struct Product {
    #[select("h2")]
    name: String,
}

/// A shop with a listing paginated over three pages.
fn shop() -> MockSite {
    let first = r#"<article><h2>Lamp</h2></article><article><h2>Desk</h2></article>
        <a href="/about">about</a><a class="next" href="/list/2">next</a>"#;
    let second = r#"<article><h2>Chair</h2></article><a class="next" href="/list/3">next</a>"#;
    MockSite::new()
        .page("https://shop.test/list/1", MockPage::html(first))
        .page("https://shop.test/list/2", MockPage::html(second))
        .page(
            "https://shop.test/list/3",
            MockPage::html("<article><h2>Shelf</h2></article>"),
        )
        .page("https://shop.test/about", MockPage::html("<p>other</p>"))
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
    let shop = shop();
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
    assert_eq!(shop.requests().len(), 3);

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
    let crawler = Crawler::new(shop(), router)
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
