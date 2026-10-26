//! The built-in handlers, alone and combined in tuples.

#![cfg(all(feature = "html", feature = "macros", feature = "testing"))]

use std::sync::Arc;

use spire::dataset::InMemDataset;
use spire::extract::Body;
use spire::extract::html::Select;
use spire::frontier::score::Keywords;
use spire::frontier::seed::Seed;
use spire::frontier::{Frontier, FrontierExt, InMemFrontier};
use spire::handler::{Collect, FollowLinks};
use spire::testing::{MockPage, MockSite};
use spire::{Crawler, Router};

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

/// A blog whose front page links to posts, the best last.
fn blog() -> MockSite {
    let front = r#"<title>Laptops</title>
        <a href="/posts/1">cooking</a>
        <a href="/posts/2">a laptop</a>
        <a href="/posts/3">laptop review</a>"#;
    MockSite::new()
        .page("https://blog.test/", MockPage::html(front))
        .page("https://blog.test/posts/1", MockPage::html("<p>post</p>"))
        .page("https://blog.test/posts/2", MockPage::html("<p>post</p>"))
        .page("https://blog.test/posts/3", MockPage::html("<p>post</p>"))
}

/// Fetches the page, which handlers otherwise only do when an extractor
/// reads the response.
async fn fetch(_: Body) {}

/// The paths a site served, in order.
fn served(site: &MockSite) -> Vec<String> {
    site.requests()
        .iter()
        .map(|uri| uri.path().to_owned())
        .collect()
}

#[tokio::test]
async fn follows_scored_links_best_first() {
    let keywords = Keywords::new(["laptop", "review"]);
    let follow = FollowLinks::new("post")
        .score_links(move |link| keywords.matches(&link.text().collect::<String>()));
    let router = Router::new().route("front", follow).route("post", fetch);
    let blog = blog();
    let crawler = Crawler::new(blog.clone(), router)
        .with_concurrency(1)
        .with_frontier(InMemFrontier::priority().dedup())
        .with_seed(Seed::get("https://blog.test/").unwrap().with_tag("front"));

    crawler.run().await.unwrap();
    assert_eq!(served(&blog), ["/", "/posts/3", "/posts/2", "/posts/1"]);
}

#[tokio::test]
async fn page_scores_add_to_link_scores() {
    let follow = FollowLinks::new("post")
        .score_page(|page| {
            if page.title().as_deref() == Some("Laptops") {
                100
            } else {
                0
            }
        })
        .score_links(|link| i32::from(link.text().any(|text| text.contains("laptop"))));
    let frontier = Arc::new(InMemFrontier::priority());
    let crawler = Crawler::new(blog(), Router::new().route("front", follow))
        .with_max_requests(1)
        .with_frontier(Arc::clone(&frontier))
        .with_seed(Seed::get("https://blog.test/").unwrap().with_tag("front"));

    crawler.run().await.unwrap();
    let mut queued: Vec<_> = Frontier::snapshot(&*frontier)
        .unwrap()
        .iter()
        .map(|seed| (seed.uri().path().to_owned(), seed.priority()))
        .collect();
    queued.sort();
    let expected = [("/posts/1", 100), ("/posts/2", 101), ("/posts/3", 101)];
    assert_eq!(
        queued,
        expected.map(|(path, priority)| (path.to_owned(), priority))
    );
}

#[tokio::test]
async fn scoring_frontier_crawls_best_first() {
    let site = MockSite::new()
        .page(
            "https://shop.test/",
            MockPage::html(
                r#"<a href="/about">about</a><a href="/laptops">laptops</a>
                <a href="/laptops/reviews">reviews</a>"#,
            ),
        )
        .page("https://shop.test/about", MockPage::html(""))
        .page("https://shop.test/laptops", MockPage::html(""))
        .page("https://shop.test/laptops/reviews", MockPage::html(""));
    let router = Router::new()
        .route("front", FollowLinks::new("page"))
        .route("page", fetch);
    let crawler = Crawler::new(site.clone(), router)
        .with_concurrency(1)
        .with_frontier(
            InMemFrontier::priority()
                .dedup()
                .score(Keywords::new(["laptop", "review"])),
        )
        .with_seed(Seed::get("https://shop.test/").unwrap().with_tag("front"));

    crawler.run().await.unwrap();
    assert_eq!(
        served(&site),
        ["/", "/laptops/reviews", "/laptops", "/about"]
    );
}
