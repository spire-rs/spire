use std::time::Duration;

use http::header::{CONTENT_TYPE, RETRY_AFTER};
use http::{HeaderValue, StatusCode};

use super::{MockPage, MockSite};
use crate::backend::{Backend, Client, StatusPolicy};
use crate::dataset::InMemDataset;
use crate::frontier::seed::Seed;
use crate::request::{Request, ResponseExt};
use crate::worker::{Outcome, from_fn};
use crate::{Context, ErrorKind, Result};

fn request(uri: &str) -> Request {
    Seed::get(uri).unwrap().into_request()
}

#[tokio::test]
async fn serves_pages_and_records_requests() {
    let site = MockSite::new().page("https://example.com", MockPage::html("<p>Home</p>"));
    let client = site.connect().await.unwrap();

    let head = client
        .fetch(&request("https://example.com/"))
        .await
        .unwrap();
    assert_eq!(head.status(), StatusCode::OK);
    assert_eq!(head.headers()[CONTENT_TYPE], "text/html; charset=utf-8");
    assert_eq!(head.final_uri().unwrap(), "https://example.com/");
    assert_eq!(client.body().await.unwrap(), "<p>Home</p>");
    assert_eq!(site.clone().requests(), ["https://example.com/"]);
}

#[tokio::test]
async fn follows_redirects() {
    let site = MockSite::new()
        .page(
            "https://example.com/a",
            MockPage::redirect("https://example.com/b"),
        )
        .page(
            "https://example.com/b",
            MockPage::redirect("https://example.com/c"),
        )
        .page("https://example.com/c", MockPage::text("c"))
        .page(
            "https://example.com/loop",
            MockPage::redirect("https://example.com/loop"),
        );
    let client = site.connect().await.unwrap();

    let head = client
        .fetch(&request("https://example.com/a"))
        .await
        .unwrap();
    assert_eq!(head.final_uri().unwrap(), "https://example.com/c");
    assert_eq!(client.body().await.unwrap(), "c");

    let error = client
        .fetch(&request("https://example.com/loop"))
        .await
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Http);
    assert_eq!(site.requests().len(), 2);
}

#[tokio::test]
async fn error_statuses_follow_the_status_policy() {
    let busy = MockPage::status(StatusCode::TOO_MANY_REQUESTS)
        .with_header(RETRY_AFTER, HeaderValue::from_static("3"));
    let site = MockSite::new().page("https://example.com/busy", busy);
    let client = site.connect().await.unwrap();

    let missing = client
        .fetch(&request("https://example.com/missing"))
        .await
        .unwrap_err();
    assert_eq!(missing.kind(), ErrorKind::Rejected);
    assert_eq!(missing.status(), Some(StatusCode::NOT_FOUND));
    assert!(
        client.body().await.is_err(),
        "no body before a fetch succeeds"
    );

    let busy = client
        .fetch(&request("https://example.com/busy"))
        .await
        .unwrap_err();
    assert_eq!(busy.kind(), ErrorKind::Http);
    assert_eq!(busy.retry_after(), Some(Duration::from_secs(3)));

    let accepting = site
        .with_status_policy(StatusPolicy::Accept)
        .connect()
        .await
        .unwrap();
    let head = accepting
        .fetch(&request("https://example.com/missing"))
        .await
        .unwrap();
    assert_eq!(head.status(), StatusCode::NOT_FOUND);
}

async fn page(cx: Context<super::MockClient>) -> Result<()> {
    let body = cx.body().await?;
    cx.dataset::<String>()?
        .write(String::from_utf8_lossy(&body).into_owned())
        .await?;
    cx.queue().follow("next").await?;
    Ok(())
}

#[tokio::test]
async fn visits_run_one_request() {
    let site = MockSite::new()
        .page(
            "https://example.com/old/",
            MockPage::redirect("https://example.com/new/"),
        )
        .page("https://example.com/new/", MockPage::html("new"));
    let visit = site
        .visit(
            Seed::get("https://example.com/old/")
                .unwrap()
                .with_tag("page"),
        )
        .with_dataset(InMemDataset::<String>::new());

    let worker = from_fn(page);
    assert!(matches!(visit.run(&worker).await, Outcome::Continue));
    assert!(matches!(visit.run(&worker).await, Outcome::Continue));

    // Links resolve against the redirected URI, one level deeper.
    let queued = visit.queued();
    assert_eq!(queued.len(), 2);
    assert_eq!(queued[0].uri(), "https://example.com/new/next");
    assert_eq!(queued[0].depth().get(), 1);
    assert_eq!(
        visit.dataset::<String>().unwrap().read_all().await.unwrap(),
        ["new", "new"]
    );
    assert_eq!(site.requests().len(), 2);
}

#[tokio::test]
async fn failed_fetches_fail_the_visit() {
    let visit = MockSite::new()
        .visit(Seed::get("https://example.com/").unwrap())
        .with_dataset(InMemDataset::<String>::new());

    let Outcome::Fail(error) = visit.run(&from_fn(page)).await else {
        panic!("a missing page fails");
    };
    assert_eq!(error.kind(), ErrorKind::Rejected);
    assert!(visit.queued().is_empty());
}

#[test]
#[should_panic(expected = "is not an absolute URI")]
fn relative_uris_panic() {
    let _ = MockSite::new().page("/relative", MockPage::text(""));
}
