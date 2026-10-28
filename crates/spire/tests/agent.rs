//! Model extraction through Markdown, router state and crawl outcomes.
#![cfg(all(feature = "agent", feature = "testing"))]

use spire::agent::cache::InMemCache;
use spire::agent::{Agent, Budget, CompletionResponse, Extract, Usage};
use spire::dataset::{Data, InMemDataset};
use spire::extract::FromRef;
use spire::frontier::seed::Seed;
use spire::handler::AgentCollect;
use spire::http::http::StatusCode;
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
    agent: Agent,
}

impl FromRef<App> for Agent {
    fn from_ref(state: &App) -> Self {
        state.agent.clone()
    }
}

async fn collect(data: Data<String>, Extract(items): Extract<Vec<String>>) -> Result<()> {
    for item in items {
        data.write(item).await?;
    }
    Ok(())
}

#[tokio::test]
async fn extracts_markdown_through_substate_and_reuses_cached_answers() {
    let provider = script(r#"{"result":["Lamp"]}"#);
    let agent = Agent::new(provider.clone()).with_cache(InMemCache::default(), "scripted/v1");
    let site = MockSite::new().page(
        "https://shop.test/",
        MockPage::html("<nav>Navigation</nav><main><h1>Lamp</h1></main>"),
    );
    let router = Router::with_state(App {
        agent: agent.clone(),
    })
    .fallback(AgentCollect::<String>::new());
    let crawler = Crawler::new(site, router)
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
    let error = Crawler::new(
        site,
        Router::with_state(agent).fallback(AgentCollect::<String>::new()),
    )
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
    let stats = Crawler::new(
        site,
        Router::with_state(Agent::new(provider.clone())).fallback(collect),
    )
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
    let router = Router::with_state(agent.clone()).fallback(AgentCollect::<String>::new());
    let crawler = Crawler::new(site.clone(), router)
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

    let stats = Crawler::new(
        site,
        Router::with_state(agent).fallback(AgentCollect::<String>::new()),
    )
    .with_seed(Seed::get("https://shop.test/").unwrap())
    .run()
    .await
    .unwrap();
    assert_eq!(stats.failed, 1);
    assert_eq!(provider.calls(), 1);
}
