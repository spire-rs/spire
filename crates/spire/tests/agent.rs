//! Model extraction through Markdown, crawler services and crawl outcomes.
#![cfg(all(feature = "agent", feature = "testing"))]

use spire::agent::cache::InMemCache;
use spire::agent::{Agent, AgentCrawlerExt, Budget, CompletionResponse, Extract, Usage};
use spire::dataset::{Data, InMemDataset};
use spire::extract::State;
use spire::frontier::seed::Seed;
use spire::handler::AgentCollect;
use spire::http::http::{StatusCode, Uri};
use spire::testing::{MockPage, MockSite, Script};
use spire::{Crawler, Result, Router};

fn script(text: &'static str) -> Script {
    Script::from_fn(move |request| {
        assert!(request.input.contains("Lamp"));
        assert!(!request.input.contains("Navigation"));
        Ok(CompletionResponse {
            text: text.into(),
            usage: Usage::tokens(10, 5).with_cost(0.0),
        })
    })
}

#[derive(Clone)]
struct App {
    prefix: String,
}

async fn app_state(State(app): State<App>) {
    assert_eq!(app.prefix, "products");
}

async fn collect(data: Data<String>, Extract(items): Extract<Vec<String>>) -> Result<()> {
    for item in items {
        data.write(item).await?;
    }
    Ok(())
}

#[tokio::test]
async fn extracts_markdown_with_independent_app_state_and_reuses_cached_answers() {
    let provider = script(r#"{"result":["Lamp"]}"#);
    let agent = Agent::new(provider.clone()).with_cache(InMemCache::default(), "scripted/v1");
    let site = MockSite::new().page(
        "https://shop.test/",
        MockPage::html("<nav>Navigation</nav><main><h1>Lamp</h1></main>"),
    );
    let router = Router::with_state(App {
        prefix: "products".into(),
    })
    .fallback((app_state, AgentCollect::<String>::new()));
    let crawler = Crawler::new(site, router)
        .with_agent(agent.clone())
        .with_dataset(InMemDataset::<String>::new())
        .with_seed(Seed::get("https://shop.test/").unwrap())
        .with_seed(Seed::get("https://shop.test/").unwrap());
    let stats = crawler.run().await.unwrap();
    assert_eq!(stats.failed, 0);
    assert_eq!(
        crawler
            .dataset::<String>()
            .unwrap()
            .read_all()
            .await
            .unwrap(),
        ["Lamp", "Lamp"]
    );
    assert_eq!(provider.calls(), 1);
    assert_eq!(agent.usage().cache_hits, 1);
}

#[tokio::test]
async fn exhausted_budget_returns_crawl_error_without_calling_provider() {
    let provider = script(r#"{"result":["Lamp"]}"#);
    let agent = Agent::new(provider.clone()).with_budget(Budget::tokens(0));
    let site = MockSite::new().page("https://shop.test/", MockPage::html("<h1>Lamp</h1>"));
    let error = Crawler::new(site, Router::new().fallback(AgentCollect::<String>::new()))
        .with_agent(agent.clone())
        .with_dataset(InMemDataset::<String>::new())
        .with_seed(Seed::get("https://shop.test/").unwrap())
        .run()
        .await
        .unwrap_err();
    assert!(error.to_string().contains("budget exhausted"), "{error}");
    assert_eq!(provider.calls(), 0);
}

#[tokio::test]
async fn rejected_http_response_never_calls_the_model() {
    let provider = script(r#"{"result":["Lamp"]}"#);
    let site = MockSite::new().page(
        "https://shop.test/",
        MockPage::status(StatusCode::NOT_FOUND),
    );
    let agent = Agent::new(provider.clone());
    let stats = Crawler::new(site, Router::new().fallback(collect))
        .with_agent(agent.clone())
        .with_dataset(InMemDataset::<String>::new())
        .with_seed(Seed::get("https://shop.test/").unwrap())
        .run()
        .await
        .unwrap();
    assert_eq!(stats.failed, 1);
    assert_eq!(provider.calls(), 0);
}

#[tokio::test]
async fn agent_collect_accepts_empty_results_and_requires_dataset_before_dispatch() {
    let site = MockSite::new().page("https://shop.test/", MockPage::html("<h1>Lamp</h1>"));
    let provider = script(r#"{"result":[]}"#);
    let agent = Agent::new(provider.clone());
    let router = Router::new().fallback(AgentCollect::<String>::new());
    let crawler = Crawler::new(site.clone(), router)
        .with_agent(agent.clone())
        .with_dataset(InMemDataset::<String>::new())
        .with_seed(Seed::get("https://shop.test/").unwrap());
    let stats = crawler.run().await.unwrap();
    assert_eq!(stats.failed, 0);
    assert_eq!(
        crawler
            .dataset::<String>()
            .unwrap()
            .read_all()
            .await
            .unwrap(),
        Vec::<String>::new()
    );
    assert_eq!(provider.calls(), 1);

    let stats = Crawler::new(site, Router::new().fallback(AgentCollect::<String>::new()))
        .with_agent(agent)
        .with_seed(Seed::get("https://shop.test/").unwrap())
        .run()
        .await
        .unwrap();
    assert_eq!(stats.failed, 1);
    assert_eq!(provider.calls(), 1);
}

#[tokio::test]
async fn missing_agent_fails_extraction_before_fetching() {
    let site = MockSite::new().page("https://shop.test/", MockPage::html("<h1>Lamp</h1>"));
    let router = Router::new().fallback(collect);
    let visit = site
        .visit(Seed::get("https://shop.test/").unwrap())
        .with_dataset(InMemDataset::<String>::new());
    let spire::worker::Outcome::Fail(error) = visit.run(&router).await else {
        panic!("missing agent should fail the handler");
    };
    assert!(error.to_string().contains("missing request extension"));
    assert!(error.to_string().contains("Agent"));
    assert_eq!(site.requests(), Vec::<Uri>::new());
}
