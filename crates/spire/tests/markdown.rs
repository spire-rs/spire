//! The `Markdown` extractor in a crawl.

#![cfg(all(feature = "markdown", feature = "testing"))]

use std::sync::{Arc, Mutex};

use spire::extract::State;
use spire::extract::html::Markdown;
use spire::frontier::seed::Seed;
use spire::testing::{MockPage, MockSite};
use spire::{Crawler, Router};

#[tokio::test]
async fn resolves_links_against_the_final_uri() {
    let site = MockSite::new()
        .page(
            "https://docs.test/old",
            MockPage::redirect("https://docs.test/guide/"),
        )
        .page(
            "https://docs.test/guide/",
            MockPage::html(
                "<main><h1>Guide</h1><p>Read <a href='install'>the install</a>.</p></main>",
            ),
        );
    let pages: Arc<Mutex<Vec<String>>> = Arc::default();
    let router = Router::with_state(Arc::clone(&pages)).fallback(
        |Markdown(markdown): Markdown, State(pages): State<Arc<Mutex<Vec<String>>>>| async move {
            pages.lock().unwrap().push(markdown);
        },
    );
    let crawler = Crawler::new(site, router).with_seed(Seed::get("https://docs.test/old").unwrap());

    let stats = crawler.run().await.unwrap();
    assert_eq!(stats.failed, 0, "{stats:?}");
    assert_eq!(
        *pages.lock().unwrap(),
        ["# Guide\n\nRead [the install](https://docs.test/guide/install)."]
    );
}
