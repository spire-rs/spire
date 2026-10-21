use std::convert::Infallible;
use std::future::{Future, ready};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tower::layer::layer_fn;
use tower::service_fn;

use crate::backend::{Backend, Client, Noop};
use crate::dataset::{Data, InMemDataset};
use crate::extract::{FromRef, Html, State, Text};
use crate::frontier::{Queue, Seed};
use crate::http::Uri;
use crate::http::header::{CONTENT_TYPE, HeaderValue};
use crate::request::{Body, Depth, Request, Response, ResponseExt, Tag};
use crate::worker::{Outcome, Worker};
use crate::{Context, Crawler, ErrorKind, Result, Router};

/// A backend serving fixed pages, as if every request was redirected to
/// `https://example.com/moved/<path>`.
#[derive(Clone)]
struct Site;

impl Backend for Site {
    type Client = Self;

    fn connect(&self) -> impl Future<Output = Result<Self>> + Send {
        ready(Ok(Self))
    }
}

impl Client for Site {
    fn fetch(self, request: Request) -> impl Future<Output = Result<Response>> + Send {
        let path = request.uri().path().to_owned();
        let html = match path.as_str() {
            "/" => {
                r#"<title>Home</title><a href="a">a</a><a href="/b#top">b</a><a href="mailto:x@y">mail</a>"#
            }
            _ => "<title>Leaf</title>",
        };

        let mut response = Response::new(Body::from(html));
        let content_type = HeaderValue::from_static("text/html; charset=utf-8");
        response.headers_mut().insert(CONTENT_TYPE, content_type);
        let moved = format!("https://example.com/moved{path}");
        response.set_final_uri(moved.parse::<Uri>().unwrap());
        ready(Ok(response))
    }
}

fn context(tag: Option<&str>) -> Context<Noop> {
    let mut seed = Seed::get("https://example.com/").unwrap();
    if let Some(tag) = tag {
        seed = seed.with_tag(tag);
    }
    Context::new(seed.into_request(), Noop)
}

async fn named(tag: Tag) -> Outcome {
    if tag.as_str() == "stop" {
        Outcome::Stop
    } else {
        Outcome::Continue
    }
}

#[tokio::test]
async fn routes_by_tag_and_falls_back() {
    let router = Router::new()
        .route("page", named)
        .route("stop", named)
        .fallback(|| async { Outcome::retry() });

    assert!(matches!(
        router.handle(context(Some("page"))).await,
        Outcome::Continue
    ));
    assert!(matches!(
        router.handle(context(Some("stop"))).await,
        Outcome::Stop
    ));
    assert!(matches!(
        router.handle(context(Some("other"))).await,
        Outcome::Retry { .. }
    ));
    assert!(matches!(
        router.handle(context(None)).await,
        Outcome::Retry { .. }
    ));
}

#[tokio::test]
async fn unrouted_requests_fail() {
    let router: Router<Noop> = Router::new().route("page", named);

    let Outcome::Fail(error) = router.handle(context(Some("typo"))).await else {
        panic!("expected a failure");
    };
    assert_eq!(error.kind(), ErrorKind::Worker);
    assert_eq!(error.to_string(), "no route for tag `typo`");
    assert!(matches!(
        router.handle(context(None)).await,
        Outcome::Fail(_)
    ));
}

#[test]
#[should_panic(expected = "tag `page` already has a route")]
fn duplicate_routes_panic() {
    let _: Router<Noop> = Router::new().route("page", named).route("page", named);
}

#[tokio::test]
async fn layers_wrap_routes_and_fallback() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let counting = layer_fn(move |route: super::Route<Noop>| {
        let counter = counter.clone();
        service_fn(move |cx| {
            counter.fetch_add(1, Ordering::SeqCst);
            tower::ServiceExt::oneshot(route.clone(), cx)
        })
    });
    let router = Router::new().route("page", named).layer(counting);

    let _ = router.handle(context(Some("page"))).await;
    let _ = router.handle(context(None)).await;
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test(start_paused = true)]
async fn layer_errors_fail_the_request() {
    async fn slow() {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }

    let timeout = tower::timeout::TimeoutLayer::new(Duration::from_secs(1));
    let router = Router::new().route("page", slow).layer(timeout);
    assert!(matches!(
        router.handle(context(Some("page"))).await,
        Outcome::Fail(_)
    ));
}

#[tokio::test]
async fn services_and_nested_routers_route() {
    let inner = Router::new().fallback(|| async { Outcome::Stop });
    let echo = service_fn(|_: Context<Noop>| async { Ok::<_, Infallible>(Outcome::retry()) });
    let router = Router::new()
        .route_service("echo", echo)
        .fallback_service(inner);

    assert!(matches!(
        router.handle(context(Some("echo"))).await,
        Outcome::Retry { .. }
    ));
    assert!(matches!(router.handle(context(None)).await, Outcome::Stop));
}

#[tokio::test]
async fn state_is_shared_with_handlers() {
    #[derive(Clone)]
    struct AppState {
        limit: u32,
    }

    struct Limit(u32);

    impl FromRef<AppState> for Limit {
        fn from_ref(state: &AppState) -> Self {
            Self(state.limit)
        }
    }

    async fn handler(State(Limit(limit)): State<Limit>, depth: Depth) -> Outcome {
        if depth.get() < limit {
            Outcome::Continue
        } else {
            Outcome::Stop
        }
    }

    let router = Router::with_state(AppState { limit: 1 }).route("page", handler);
    assert!(matches!(
        router.handle(context(Some("page"))).await,
        Outcome::Continue
    ));
}

#[tokio::test]
async fn rejected_extractors_fail_and_optional_ones_do_not() {
    async fn needs_tag(_: Tag) {}
    async fn maybe_tag(tag: Option<Tag>) -> Outcome {
        if tag.is_none() {
            Outcome::Stop
        } else {
            Outcome::Continue
        }
    }

    let router: Router<Noop> = Router::new().fallback(needs_tag);
    assert!(matches!(
        router.handle(context(None)).await,
        Outcome::Fail(_)
    ));

    let router: Router<Noop> = Router::new().fallback(maybe_tag);
    assert!(matches!(router.handle(context(None)).await, Outcome::Stop));
}

#[tokio::test]
async fn crawls_and_resolves_links_after_redirects() {
    async fn page(uri: Uri, queue: Queue, data: Data<String>, page: Html) -> Result<()> {
        let title = page.title().unwrap_or_default();
        data.write(format!("{} {title}", uri.path())).await?;
        queue.follow_all(page.links()).await?;
        Ok(())
    }

    let crawler = Crawler::new(Site, Router::new().route("page", page))
        .with_dataset(InMemDataset::<String>::new())
        .with_seed(Seed::get("https://example.com/").unwrap().with_tag("page"));

    let stats = crawler.run().await.unwrap();
    assert_eq!((stats.requests, stats.failed), (3, 0));

    // `a` resolves against the redirected `/moved/`, `/b#top` loses its
    // fragment, `mailto:` is skipped, and followed links keep the tag.
    let mut seen = crawler
        .dataset::<String>()
        .unwrap()
        .read_all()
        .await
        .unwrap();
    seen.sort();
    assert_eq!(seen, ["/ Home", "/b Leaf", "/moved/a Leaf"]);
}

#[tokio::test]
async fn text_extractor_reads_the_body() {
    async fn leaf(Text(body): Text) -> Outcome {
        if body.contains("Leaf") {
            Outcome::Stop
        } else {
            Outcome::Continue
        }
    }

    let request = Seed::get("https://example.com/leaf")
        .unwrap()
        .into_request();
    let router = Router::new().fallback(leaf);
    assert!(matches!(
        router.handle(Context::new(request, Site)).await,
        Outcome::Stop
    ));
}
