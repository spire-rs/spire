//! The browser backend against a real WebDriver server and httpbin.org.
//!
//! Skipped unless `SPIRE_WEBDRIVER` names a WebDriver server with BiDi, e.g.
//! `SPIRE_WEBDRIVER=http://127.0.0.1:4444` with `chromedriver --port=4444`.
//! Sessions are dropped before the backend is closed: a session dropped after
//! the pool closed is quit synchronously, which blocks.

use spire_core::ErrorKind;
use spire_core::backend::{Backend, Client, StatusPolicy};
use spire_core::frontier::seed::Seed;
use spire_core::http::header::{CONTENT_TYPE, HeaderValue};
use spire_core::http::{Method, StatusCode};
use spire_core::request::Request;
use spire_thirtyfour::{BrowserBackend, BrowserBuilder};

fn builder() -> Option<BrowserBuilder> {
    let url = std::env::var("SPIRE_WEBDRIVER").ok()?;
    Some(BrowserBackend::builder().remote(url).max_sessions(1))
}

fn get(uri: &str) -> Request {
    Seed::get(uri).unwrap().into_request()
}

#[tokio::test]
async fn reports_real_statuses_and_follows_redirects() {
    let Some(builder) = builder() else { return };
    let backend = builder.build().unwrap();

    let session = backend.connect().await.unwrap();
    let error = session
        .fetch(&get("https://httpbin.org/status/404"))
        .await
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Rejected, "{error:?}");
    assert_eq!(error.status(), Some(StatusCode::NOT_FOUND));
    drop(session);

    let session = backend.connect().await.unwrap();
    let response = session
        .fetch(&get("https://httpbin.org/redirect/2"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[CONTENT_TYPE], "text/html; charset=utf-8");
    let final_uri = spire_core::request::ResponseExt::final_uri(&response);
    assert_eq!(final_uri.unwrap(), "https://httpbin.org/get");
    drop(session);

    backend.close().await;
}

#[tokio::test]
async fn accepts_error_statuses_when_asked() {
    let Some(builder) = builder() else { return };
    let backend = builder.status_policy(StatusPolicy::Accept).build().unwrap();

    let session = backend.connect().await.unwrap();
    let head = session
        .fetch(&get("https://httpbin.org/status/503"))
        .await
        .unwrap();
    assert_eq!(head.status(), StatusCode::SERVICE_UNAVAILABLE);
    drop(session);

    backend.close().await;
}

#[tokio::test]
async fn sends_methods_headers_and_bodies() {
    let Some(builder) = builder() else { return };
    let backend = builder.build().unwrap();

    let form = HeaderValue::from_static("application/x-www-form-urlencoded");
    let post = Seed::new(Method::POST, "https://httpbin.org/post".parse().unwrap())
        .unwrap()
        .with_header(CONTENT_TYPE, form)
        .with_header("x-spire", HeaderValue::from_static("bidi"))
        .with_body("probe=1");

    let session = backend.connect().await.unwrap();
    session.fetch(&post.into_request()).await.unwrap();
    let body = String::from_utf8(session.body().await.unwrap().to_vec()).unwrap();
    assert!(body.contains(r#""probe": "1""#), "{body}");
    assert!(body.contains(r#""X-Spire": "bidi""#), "{body}");
    drop(session);

    backend.close().await;
}

#[tokio::test]
async fn isolates_requests_unless_state_is_kept() {
    for (keep, expected) in [(false, false), (true, true)] {
        let Some(mut builder) = builder() else { return };
        if keep {
            builder = builder.keep_session_state();
        }
        let backend = builder.build().unwrap();

        let session = backend.connect().await.unwrap();
        session
            .fetch(&get("https://httpbin.org/cookies/set?probe=1"))
            .await
            .unwrap();
        drop(session);

        let session = backend.connect().await.unwrap();
        session
            .fetch(&get("https://httpbin.org/cookies"))
            .await
            .unwrap();
        let source = session.page_source().await.unwrap();
        assert_eq!(
            source.contains("probe"),
            expected,
            "keep = {keep}: {source}"
        );
        drop(session);

        backend.close().await;
    }
}

#[tokio::test]
async fn reads_the_page_as_currently_rendered() {
    let Some(builder) = builder() else { return };
    let backend = builder.build().unwrap();

    let session = backend.connect().await.unwrap();
    session
        .fetch(&get("https://httpbin.org/html"))
        .await
        .unwrap();
    let loaded = String::from_utf8(session.body().await.unwrap().to_vec()).unwrap();
    assert!(loaded.contains("Moby-Dick"), "{loaded}");

    // Changes made after loading show in the body, without navigating again.
    let script = "document.body.innerHTML = '<p>changed</p>'";
    session.execute(script, Vec::new()).await.unwrap();
    let changed = String::from_utf8(session.body().await.unwrap().to_vec()).unwrap();
    assert!(changed.contains("<p>changed</p>"), "{changed}");
    drop(session);

    backend.close().await;
}
