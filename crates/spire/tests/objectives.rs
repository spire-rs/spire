//! Objective-based discovery and stopping through the real handler pipeline.
#![cfg(all(feature = "agent", feature = "testing"))]

use spire::agent::objective::{AgentFrontierExt, Finished, Objective};
use spire::agent::schemars::JsonSchema;
use spire::agent::serde::{Deserialize, Serialize};
use spire::agent::serde_json::{Value, json};
use spire::agent::testing::Script;
use spire::agent::{Agent, Budget, CompletionResponse, Usage};
use spire::dataset::InMemDataset;
use spire::frontier::seed::Seed;
use spire::frontier::{FrontierExt, InMemFrontier};
use spire::handler::{AgentCollect, FollowRelevant};
use spire::testing::{MockPage, MockSite};
use spire::{Crawler, Router};

#[derive(Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(crate = "spire::agent::serde")]
#[schemars(crate = "spire::agent::schemars")]
struct Plan {
    name: String,
    price: String,
}

fn script() -> Script {
    Script::from_fn(|request| {
        let text = if request.system.starts_with("Rate each link") {
            let input: Value = spire::agent::serde_json::from_str(&request.input).unwrap();
            let scores = input["links"]
                .as_array()
                .unwrap()
                .iter()
                .map(|link| {
                    let url = link["url"].as_str().unwrap();
                    assert!(!url.contains("hidden"));
                    if url.contains("pricing") {
                        95
                    } else if url.contains("faq") {
                        70
                    } else {
                        0
                    }
                })
                .collect::<Vec<_>>();
            json!({"scores":scores}).to_string()
        } else if request.system.starts_with("Judge whether") {
            r#"{"met":true}"#.into()
        } else if request.input.contains("Basic: $9") {
            r#"{"result":[{"name":"Basic","price":"$9"}]}"#.into()
        } else {
            r#"{"result":[]}"#.into()
        };
        Ok(CompletionResponse {
            text,
            usage: Usage::tokens(10, 5).with_cost(0.01),
        })
    })
}

fn site() -> MockSite {
    MockSite::new()
        .page("https://shop.test/", MockPage::html(r#"<main><a href="/faq">FAQ</a><a href="/pricing">Plans</a><a href="/irrelevant">Careers</a><a hidden href="/hidden">Pricing trap</a></main>"#))
        .page("https://shop.test/pricing", MockPage::html("<main>Basic: $9</main>"))
        .page("https://shop.test/faq", MockPage::html("<main>FAQ</main>"))
}

#[tokio::test]
async fn follows_visible_relevant_links_by_priority_then_stops_when_met() {
    let config = Objective::new("find plan prices")
        .relevant("pricing and billing")
        .done_when("Basic has a price")
        .check_every(1);
    let provider = script();
    let objective = config.start(Agent::new(provider.clone()));
    let site = site();
    let router = Router::with_state(objective.clone()).fallback((
        AgentCollect::<Plan>::new().report_to(&objective),
        FollowRelevant::new("page").same_host(),
    ));
    let crawler = Crawler::new(site.clone(), router)
        .with_frontier(InMemFrontier::priority().dedup().until(&objective))
        .with_dataset(InMemDataset::<Plan>::new())
        .with_concurrency(1)
        .with_seed(Seed::get("https://shop.test/").unwrap());
    let stats = objective.run(&crawler).await.unwrap();
    assert_eq!(stats.failed, 0);
    assert_eq!(objective.finished(), Some(Finished::Met));
    assert_eq!(
        site.requests()
            .iter()
            .map(spire::http::http::Uri::path)
            .collect::<Vec<_>>(),
        ["/", "/pricing"]
    );
    assert_eq!(
        crawler.dataset::<Plan>().unwrap().read_all().await.unwrap(),
        [Plan {
            name: "Basic".into(),
            price: "$9".into()
        }]
    );
    assert_eq!(provider.calls(), 4);
}

#[tokio::test]
async fn item_target_stops_host_parked_requests_without_requiring_follow_relevant() {
    let objective = Objective::new("find a plan")
        .target_items(1)
        .start(Agent::new(script()));
    let mut site = site();
    for id in 0..10 {
        site = site.page(
            &format!("https://shop.test/pricing?q={id}"),
            MockPage::html("<main>Basic: $9</main>"),
        );
    }
    let mut crawler = Crawler::new(
        site.clone(),
        Router::with_state(objective.clone())
            .fallback(AgentCollect::<Plan>::new().report_to(&objective)),
    )
    .with_frontier(InMemFrontier::fifo().until(&objective))
    .with_dataset(InMemDataset::<Plan>::new())
    .with_concurrency(8)
    .with_host_concurrency(1);
    for id in 0..10 {
        crawler =
            crawler.with_seed(Seed::get(format!("https://shop.test/pricing?q={id}")).unwrap());
    }
    let stats = objective.run(&crawler).await.unwrap();
    assert_eq!(stats.requests, 1);
    assert_eq!(site.requests().len(), 1);
    assert_eq!(objective.finished(), Some(Finished::TargetItems));
}

#[tokio::test]
async fn budget_exhaustion_preserves_crawl_error_and_objective_reason() {
    let objective =
        Objective::new("plans").start(Agent::new(script()).with_budget(Budget::tokens(0)));
    let crawler = Crawler::new(
        site(),
        Router::with_state(objective.clone()).fallback(FollowRelevant::new("page")),
    )
    .with_frontier(InMemFrontier::priority().dedup().until(&objective))
    .with_seed(Seed::get("https://shop.test/").unwrap());
    let error = objective.run(&crawler).await.unwrap_err();
    assert!(error.to_string().contains("budget exhausted"), "{error}");
    assert_eq!(objective.finished(), Some(Finished::Budget));
}

#[tokio::test]
async fn low_concurrency_finishes_small_batches_and_exhausts_without_invented_items() {
    let objective = Objective::new("plans").start(Agent::new(script()));
    let site = MockSite::new()
        .page(
            "https://shop.test/",
            MockPage::html("<a href='/faq'>FAQ</a>"),
        )
        .page(
            "https://shop.test/faq",
            MockPage::html("<p>No plans here</p>"),
        );
    let crawler = Crawler::new(
        site.clone(),
        Router::with_state(objective.clone()).fallback((
            AgentCollect::<Plan>::new().report_to(&objective),
            FollowRelevant::new("page"),
        )),
    )
    .with_frontier(InMemFrontier::priority().dedup().until(&objective))
    .with_dataset(InMemDataset::<Plan>::new())
    .with_concurrency(1)
    .with_seed(Seed::get("https://shop.test/").unwrap());
    objective.run(&crawler).await.unwrap();
    assert_eq!(objective.finished(), Some(Finished::Exhausted));
    assert_eq!(site.requests().len(), 2);
    assert_eq!(objective.progress().items, 0);
}

#[tokio::test]
async fn failed_dataset_writes_do_not_report_items_or_evidence() {
    struct Reject;
    impl spire::dataset::Dataset<Plan> for Reject {
        fn write(&self, _: Plan) -> impl Future<Output = spire::Result<()>> + Send {
            std::future::ready(Err(
                spire::Error::new(spire::ErrorKind::Dataset).with_message("write refused")
            ))
        }

        fn read(&self) -> impl Future<Output = spire::Result<Option<Plan>>> + Send {
            std::future::ready(Ok(None))
        }
    }
    let objective = Objective::new("plans")
        .target_items(1)
        .start(Agent::new(script()));
    let crawler = Crawler::new(
        site(),
        Router::with_state(objective.clone())
            .fallback(AgentCollect::<Plan>::new().report_to(&objective)),
    )
    .with_frontier(InMemFrontier::fifo().until(&objective))
    .with_dataset(Reject)
    .with_seed(Seed::get("https://shop.test/pricing").unwrap());
    let stats = objective.run(&crawler).await.unwrap();
    assert_eq!(stats.failed, 1);
    assert_eq!(objective.progress().items, 0);
    assert_eq!(objective.progress().record, Vec::<Value>::new());
    assert_eq!(objective.finished(), Some(Finished::Exhausted));
}

#[tokio::test]
async fn explicit_collection_counts_stored_duplicates_and_survives_handler_cloning() {
    let objective = Objective::new("plans")
        .target_items(2)
        .start(Agent::new(script()));
    let site = MockSite::new()
        .page(
            "https://shop.test/one",
            MockPage::html("<main>Basic: $9</main>"),
        )
        .page(
            "https://shop.test/two",
            MockPage::html("<main>Basic: $9</main>"),
        );
    let handler = AgentCollect::<Plan>::new().report_to(&objective);
    let crawler = Crawler::new(
        site,
        Router::with_state(objective.clone()).fallback(handler.clone()),
    )
    .with_frontier(InMemFrontier::fifo().until(&objective))
    .with_dataset(InMemDataset::<Plan>::new())
    .with_concurrency(1)
    .with_seed(Seed::get("https://shop.test/one").unwrap())
    .with_seed(Seed::get("https://shop.test/two").unwrap());
    objective.run(&crawler).await.unwrap();
    assert_eq!(objective.progress().items, 2);
    assert_eq!(
        crawler
            .dataset::<Plan>()
            .unwrap()
            .read_all()
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(objective.finished(), Some(Finished::TargetItems));
}
