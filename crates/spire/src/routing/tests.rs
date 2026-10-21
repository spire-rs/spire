use std::future::{Future, ready};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use bytes::Bytes;

#[cfg(feature = "html")]
use crate::backend::Backend;
use crate::backend::{Client, Noop};
use crate::extract::{FromRef, State, Text};
use crate::frontier::seed::Seed;
use crate::request::http::Uri;
use crate::request::http::header::{CONTENT_TYPE, HeaderValue};
use crate::request::{Depth, Request, Response, ResponseExt, Tag, Tags};
use crate::worker::{Outcome, Worker};
use crate::{Context, ErrorKind, Result, Router};

/// A backend serving fixed pages, as if every request was redirected to
/// `https://example.com/moved/<path>`.
#[cfg(feature = "html")]
#[derive(Clone)]
struct Site;

#[cfg(feature = "html")]
impl Backend for Site {
    type Client = Visit;

    fn connect(&self) -> impl Future<Output = Result<Visit>> + Send {
        ready(Ok(Visit(Mutex::new(""))))
    }
}

/// A request to [`Site`], keeping the page it was served.
struct Visit(Mutex<&'static str>);

impl Client for Visit {
    fn fetch(&self, request: &Request) -> impl Future<Output = Result<Response<()>>> + Send {
        let path = request.uri().path().to_owned();
        *self.0.lock().unwrap() = match path.as_str() {
            "/" => {
                r#"<title>Home</title><a href="a">a</a><a href="/b#top">b</a><a href="mailto:x@y">mail</a>"#
            }
            _ => "<title>Leaf</title>",
        };

        let mut head = Response::new(());
        let content_type = HeaderValue::from_static("text/html; charset=utf-8");
        head.headers_mut().insert(CONTENT_TYPE, content_type);
        let moved = format!("https://example.com/moved{path}");
        head.set_final_uri(moved.parse::<Uri>().unwrap());
        ready(Ok(head))
    }

    fn body(&self) -> impl Future<Output = Result<Bytes>> + Send {
        ready(Ok(Bytes::from_static(self.0.lock().unwrap().as_bytes())))
    }
}

fn context(tag: Option<&str>) -> Context<Noop> {
    context_at("https://example.com/", tag)
}

fn context_at(uri: &str, tag: Option<&str>) -> Context<Noop> {
    let mut seed = Seed::get(uri).unwrap();
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
    let Outcome::Fail(error) = router.handle(context(None)).await else {
        panic!("expected a failure");
    };
    assert_eq!(
        error.to_string(),
        "no path or fallback route for untagged `/`"
    );
}

#[test]
#[should_panic(expected = "tag `page` already has a route")]
fn duplicate_routes_panic() {
    let _: Router<Noop> = Router::new().route("page", named).route("page", named);
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
#[cfg(feature = "html")]
async fn crawls_and_resolves_links_after_redirects() {
    use crate::Crawler;
    use crate::dataset::{Data, InMemDataset};
    use crate::extract::Html;
    use crate::frontier::queue::Queue;

    async fn page(uri: Uri, queue: Queue, data: Data<String>, page: Html) -> Result<()> {
        let title = page.title().unwrap_or_default();
        data.write(format!("{} {title}", uri.path())).await?;
        queue.follow_all(page.links()).await?;
        Ok(())
    }

    let router = Router::new().route("home", page).fallback(page);
    let crawler = Crawler::new(Site, router)
        .with_dataset(InMemDataset::<String>::new())
        .with_seed(Seed::get("https://example.com/").unwrap().with_tag("home"));

    let stats = crawler.run().await.unwrap();
    assert_eq!((stats.requests, stats.failed), (3, 0));

    // `a` resolves against the redirected `/moved/`, `/b#top` loses its
    // fragment, `mailto:` is skipped, and untagged links reach the fallback.
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
        router
            .handle(Context::new(request, Visit(Mutex::new(""))))
            .await,
        Outcome::Stop
    ));
}

/// A hand-written [`Tags`] set, as `#[derive(Tags)]` generates it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    List,
    Item,
}

impl From<Page> for Tag {
    fn from(page: Page) -> Self {
        Self::from_static(match page {
            Page::List => "list",
            Page::Item => "item",
        })
    }
}

impl Tags for Page {
    const ALL: &'static [Self] = &[Self::List, Self::Item];
}

#[tokio::test]
async fn untagged_requests_route_by_path() {
    let outcome = |outcome: fn() -> Outcome| move || ready(outcome());
    let router = Router::new()
        .route("item", outcome(|| Outcome::Continue))
        .route_path("/items/*", outcome(|| Outcome::Stop))
        .route_path("/items/**", outcome(Outcome::retry))
        .fallback(outcome(|| Outcome::Fail(crate::Error::worker("fallback"))));
    let handle = |uri: &str, tag: Option<&str>| router.handle(context_at(uri, tag));

    // The first matching pattern wins, the query is ignored, and a tag
    // always wins over the path.
    let outcome = handle("https://example.com/items/1?page=2", None).await;
    assert!(matches!(outcome, Outcome::Stop));
    let outcome = handle("https://example.com/items/1/reviews", None).await;
    assert!(matches!(outcome, Outcome::Retry { .. }));
    let outcome = handle("https://example.com/items/1", Some("item")).await;
    assert!(matches!(outcome, Outcome::Continue));
    let outcome = handle("https://example.com/other", None).await;
    assert!(matches!(outcome, Outcome::Fail(_)));

    // Unknown tags skip the path routes, so typos still reach the fallback.
    let outcome = handle("https://example.com/items/1", Some("typo")).await;
    assert!(matches!(outcome, Outcome::Fail(_)));
}

#[test]
#[should_panic(expected = "path `/items/*` already has a route")]
fn duplicate_path_routes_panic() {
    let a = Router::<Noop>::new().route_path("/items/*", named);
    let b = Router::new().route_path("/items/*", named);
    let _ = a.merge(b);
}

#[tokio::test]
async fn enum_tags_route() {
    let router = Router::new()
        .route(Page::List, || async { Outcome::Continue })
        .route(Page::Item, || async { Outcome::Stop })
        .require::<Page>();
    assert!(matches!(
        router.handle(context(Some("list"))).await,
        Outcome::Continue
    ));
    assert!(matches!(
        router.handle(context(Some("item"))).await,
        Outcome::Stop
    ));
}

#[test]
#[should_panic(expected = "no route for tag `item`")]
fn require_names_missing_tags() {
    let _ = Router::<Noop>::new()
        .route(Page::List, named)
        .require::<Page>();
}

/// A client counting its fetches, serving `page` as the body.
struct Counted(Arc<AtomicUsize>);

impl Client for Counted {
    fn fetch(&self, _request: &Request) -> impl Future<Output = Result<Response<()>>> + Send {
        self.0.fetch_add(1, Ordering::SeqCst);
        ready(Ok(Response::new(())))
    }

    fn body(&self) -> impl Future<Output = Result<Bytes>> + Send {
        ready(Ok(Bytes::from_static(b"page")))
    }
}

#[tokio::test]
async fn handler_tuples_share_one_fetch_and_stop_early() {
    let calls = Arc::new(AtomicUsize::new(0));
    let fetches = Arc::new(AtomicUsize::new(0));
    let handler = |outcome: fn() -> Outcome| {
        let calls = calls.clone();
        move |Text(body): Text| {
            let calls = calls.clone();
            async move {
                assert_eq!(body, "page");
                calls.fetch_add(1, Ordering::SeqCst);
                outcome()
            }
        }
    };
    let context = || {
        let request = Seed::get("https://example.com/").unwrap().with_tag("page");
        Context::new(request.into_request(), Counted(fetches.clone()))
    };

    // Every handler reads the body, and the request is fetched once.
    let all = (
        handler(|| Outcome::Continue),
        handler(|| Outcome::Continue),
        handler(|| Outcome::Continue),
    );
    let router: Router<Counted> = Router::new().route("page", all);
    assert!(matches!(router.handle(context()).await, Outcome::Continue));
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert_eq!(fetches.load(Ordering::SeqCst), 1);

    // The first outcome other than `Continue` ends the tuple.
    calls.store(0, Ordering::SeqCst);
    let stopping = (
        handler(|| Outcome::Continue),
        handler(|| Outcome::Stop),
        handler(|| Outcome::Continue),
    );
    let router: Router<Counted> = Router::new().route("page", stopping);
    assert!(matches!(router.handle(context()).await, Outcome::Stop));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}
