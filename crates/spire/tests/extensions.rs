//! Crawler services remain independent of application state and middleware overrides.

use spire::backend::Noop;
use spire::extract::{Extension, FromContext, State};
use spire::frontier::seed::Seed;
use spire::http::Request;
use spire::worker::{Worker, from_fn};
use spire::{Context, Crawler, Router};

#[tokio::test]
async fn required_and_optional_extensions_are_checked_by_type() {
    let cx = Context::new(Request::default(), Noop).with_extension(5_usize);
    assert_eq!(
        Extension::<usize>::from_context(&cx, &()).await.unwrap().0,
        5
    );
    let error = Extension::<String>::from_context(&cx, &())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("missing request extension"));
    assert!(error.to_string().contains("String"));
    assert!(
        Option::<Extension<String>>::from_context(&cx, &())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn middleware_overrides_services_without_replacing_application_state() {
    async fn handler(State(state): State<String>, Extension(value): Extension<usize>) {
        assert_eq!(state, "application");
        assert_eq!(value, 9);
    }
    let router = Router::with_state("application".to_owned()).fallback(handler);
    let worker = from_fn(move |mut cx: Context<Noop>| {
        let router = router.clone();
        async move {
            assert_eq!(cx.request().extensions().get::<usize>(), Some(&5));
            cx.request_mut().extensions_mut().insert(9_usize);
            router.handle(cx).await
        }
    });
    let stats = Crawler::new(Noop, worker)
        .with_extension(5_usize)
        .with_seed(Seed::get("https://example.com/").unwrap())
        .run()
        .await
        .unwrap();
    assert_eq!(stats.failed, 0);
}

#[cfg(feature = "testing")]
#[tokio::test]
async fn visits_accept_services_without_crawler_setup() {
    use spire::testing::{MockPage, MockSite};

    async fn handler(Extension(value): Extension<usize>) {
        assert_eq!(value, 5);
    }
    let router = Router::new().fallback(handler);
    let site = MockSite::new().page("https://example.com/", MockPage::html("hello"));
    let visit = site
        .visit(Seed::get("https://example.com/").unwrap())
        .with_extension(5_usize);
    assert!(matches!(
        visit.run(&router).await,
        spire::worker::Outcome::Continue
    ));
}
