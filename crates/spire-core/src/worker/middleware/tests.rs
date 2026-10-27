use std::sync::{Arc, Mutex};

use ::http::HeaderValue;
use ::http::header::{ACCEPT, USER_AGENT};

use super::{Next, SetHeader, from_fn as middleware};
use crate::frontier::seed::Seed;
use crate::testing::{MockPage, MockSession, MockSite};
use crate::worker::{Outcome, WorkerExt, from_fn};
use crate::{Context, Error, Result};

fn site() -> MockSite {
    MockSite::new()
        .page("https://example.com/", MockPage::text("page"))
        .page(
            "https://example.com/captcha",
            MockPage::text("").with_header("x-captcha", HeaderValue::from_static("1")),
        )
}

fn seed(uri: &str) -> Seed {
    Seed::get(uri).unwrap()
}

/// A worker recording the header `name` of each request it handles.
fn recording(
    name: &'static str,
    seen: &Arc<Mutex<Vec<String>>>,
) -> impl crate::worker::Worker<MockSession> {
    let seen = Arc::clone(seen);
    from_fn(move |cx: Context<MockSession>| {
        let value = cx
            .request()
            .headers()
            .get(name)
            .map(|value| value.to_str().unwrap().to_owned());
        seen.lock().unwrap().push(value.unwrap_or_default());
        async {}
    })
}

#[tokio::test]
async fn the_last_layer_runs_first() {
    let order = Arc::new(Mutex::new(Vec::new()));
    let step = |name: &'static str| {
        let order = Arc::clone(&order);
        middleware(move |cx: Context<MockSession>, next: Next<MockSession>| {
            order.lock().unwrap().push(name);
            next.run(cx)
        })
    };

    let worker = from_fn(|_: Context<MockSession>| async {})
        .layer(step("inner"))
        .layer(step("outer"));
    let outcome = site()
        .visit(seed("https://example.com/"))
        .run(&worker)
        .await;
    assert!(matches!(outcome, Outcome::Continue));
    assert_eq!(*order.lock().unwrap(), ["outer", "inner"]);
}

#[tokio::test]
async fn set_header_keeps_the_seeds_own() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let agent = HeaderValue::from_static("a");
    let worker = recording("user-agent", &seen).layer(SetHeader::new(USER_AGENT, agent));

    let site = site();
    let outcome = site.visit(seed("https://example.com/")).run(&worker).await;
    assert!(matches!(outcome, Outcome::Continue));
    let own = seed("https://example.com/").with_header(USER_AGENT, HeaderValue::from_static("own"));
    let outcome = site.visit(own).run(&worker).await;
    assert!(matches!(outcome, Outcome::Continue));
    assert_eq!(*seen.lock().unwrap(), ["a", "own"]);
}

#[tokio::test]
async fn headers_reach_the_client() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let fetching = {
        let seen = Arc::clone(&seen);
        middleware(move |cx: Context<MockSession>, next: Next<MockSession>| {
            let seen = Arc::clone(&seen);
            async move {
                // Fetched here, before the worker: the request as sent.
                cx.response().await?;
                let accept = cx.request().headers()[ACCEPT].to_str().unwrap().to_owned();
                seen.lock().unwrap().push(accept);
                Ok::<_, Error>(next.run(cx).await)
            }
        })
    };
    let worker = from_fn(|_: Context<MockSession>| async {})
        .layer(fetching)
        .layer(SetHeader::new(
            ACCEPT,
            HeaderValue::from_static("text/html"),
        ));

    let site = site();
    let outcome = site.visit(seed("https://example.com/")).run(&worker).await;
    assert!(matches!(outcome, Outcome::Continue));
    assert_eq!(*seen.lock().unwrap(), ["text/html"]);
}

/// Retries pages answering with a captcha, without running the worker.
async fn captcha(cx: Context<MockSession>, next: Next<MockSession>) -> Result<Outcome> {
    if cx.response().await?.headers().contains_key("x-captcha") {
        return Ok(Outcome::Retry { after: None });
    }
    Ok(next.run(cx).await)
}

#[tokio::test]
async fn middleware_checks_the_response_without_fetching_twice() {
    let handled = Arc::new(Mutex::new(Vec::new()));
    let worker = recording("accept", &handled).layer(middleware(captcha));
    let site = site();

    let outcome = site
        .visit(seed("https://example.com/captcha"))
        .run(&worker)
        .await;
    assert!(matches!(outcome, Outcome::Retry { after: None }));
    assert!(handled.lock().unwrap().is_empty(), "the worker did not run");

    let page = from_fn(|cx: Context<MockSession>| async move { cx.body().await.map(drop) })
        .layer(middleware(captcha));
    let outcome = site.visit(seed("https://example.com/")).run(&page).await;
    assert!(matches!(outcome, Outcome::Continue));
    assert_eq!(site.requests().len(), 2, "one fetch per visit");
}

#[tokio::test]
async fn middleware_can_answer_without_fetching() {
    let skip = middleware(|_: Context<MockSession>, _: Next<MockSession>| async { Outcome::Stop });
    let worker =
        from_fn(|cx: Context<MockSession>| async move { cx.body().await.map(drop) }).layer(skip);

    let site = site();
    let outcome = site.visit(seed("https://example.com/")).run(&worker).await;
    assert!(matches!(outcome, Outcome::Stop));
    assert_eq!(site.requests(), [""; 0]);
}

#[tokio::test]
async fn failed_middleware_fails_the_request() {
    let worker = from_fn(|_: Context<MockSession>| async {}).layer(middleware(captcha));
    let outcome = site()
        .visit(seed("https://example.com/missing"))
        .run(&worker)
        .await;
    assert!(matches!(outcome, Outcome::Fail(_)));
}
