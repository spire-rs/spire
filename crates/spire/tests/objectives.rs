//! Objective middleware composes ordinary handlers without changing the crawler.
#![cfg(all(feature = "agent", feature = "testing"))]

#[path = "fixtures/objectives.rs"]
mod fixtures;

use spire::agent::objective::{Objective, ObjectiveExt, StopPolicy, StopReason};
use spire::agent::schemars::JsonSchema;
use spire::agent::serde::{Deserialize, Serialize};
use spire::agent::{Agent, AgentCrawlerExt, Budget};
use spire::dataset::InMemDataset;
use spire::extract::State;
use spire::frontier::seed::Seed;
use spire::frontier::{FrontierExt, InMemFrontier};
use spire::handler::{AgentCollect, FollowRelevantLinks};
use spire::{Crawler, Router};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(crate = "spire::agent::serde")]
#[schemars(crate = "spire::agent::schemars")]
struct Plan {
    name: String,
    price: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(crate = "spire::agent::serde")]
#[schemars(crate = "spire::agent::schemars")]
struct Contact {
    email: String,
}

#[tokio::test]
async fn merges_relevance_and_shares_fetches_while_objectives_finish_independently() {
    async fn state(State(label): State<String>) {
        assert_eq!(label, "application");
    }
    let provider = fixtures::provider();
    let site = fixtures::site();
    let router = Router::with_state("application".to_owned())
        .fallback((
            state,
            AgentCollect::<Plan>::new(),
            AgentCollect::<Contact>::new(),
            FollowRelevantLinks::new("page").same_host(),
        ))
        .with_objective(Objective::<Plan>::new("plans").target_items(1))
        .with_objective(Objective::<Contact>::new("contacts").target_items(2));
    let plans = router.objective::<Plan>().unwrap();
    let contacts = router.objective::<Contact>().unwrap();
    let crawler = Crawler::new(site.clone(), router)
        .with_agent(Agent::new(provider.clone()))
        .with_frontier(InMemFrontier::priority().dedup())
        .with_dataset(InMemDataset::<Plan>::new())
        .with_dataset(InMemDataset::<Contact>::new())
        .with_concurrency(1)
        .with_seed(Seed::get("https://shop.test/").unwrap());
    assert_eq!(crawler.run().await.unwrap().failed, 0);
    assert_eq!(
        site.requests()
            .iter()
            .map(spire::http::http::Uri::path)
            .collect::<Vec<_>>(),
        ["/", "/shared", "/contact"]
    );
    assert_eq!(plans.stop_reason(), Some(StopReason::TargetItems));
    assert_eq!(contacts.stop_reason(), Some(StopReason::TargetItems));
    assert_eq!(plans.progress().items, 1);
    assert_eq!(contacts.progress().items, 2);
    assert_eq!(plans.progress().pages, 2);
    assert_eq!(contacts.progress().pages, 3);
    assert_eq!(provider.calls(), 7);
}

#[tokio::test]
async fn any_policy_preserves_other_objectives_unmet() {
    let site = fixtures::site();
    let router = Router::new()
        .fallback((AgentCollect::<Plan>::new(), AgentCollect::<Contact>::new()))
        .with_objective(Objective::<Plan>::new("plans").target_items(1))
        .with_objective(Objective::<Contact>::new("contacts").target_items(5))
        .stop_when(StopPolicy::Any);
    let plans = router.objective::<Plan>().unwrap();
    let contacts = router.objective::<Contact>().unwrap();
    let crawler = Crawler::new(site.clone(), router)
        .with_agent(Agent::new(fixtures::provider()))
        .with_dataset(InMemDataset::<Plan>::new())
        .with_dataset(InMemDataset::<Contact>::new())
        .with_concurrency(1)
        .with_seed(Seed::get("https://shop.test/shared").unwrap())
        .with_seed(Seed::get("https://shop.test/contact").unwrap());
    assert_eq!(crawler.run().await.unwrap().requests, 1);
    assert_eq!(plans.stop_reason(), Some(StopReason::TargetItems));
    assert_eq!(contacts.stop_reason(), None);
}

#[tokio::test]
async fn work_exhaustion_leaves_goals_unmet_and_more_seeds_continue_progress() {
    let router = Router::new()
        .fallback(AgentCollect::<Plan>::new())
        .with_objective(Objective::<Plan>::new("plans").target_items(2));
    let status = router.objective::<Plan>().unwrap();
    let crawler = Crawler::new(fixtures::site(), router)
        .with_agent(Agent::new(fixtures::provider()))
        .with_dataset(InMemDataset::<Plan>::new())
        .with_seed(Seed::get("https://shop.test/pricing").unwrap());
    crawler.run().await.unwrap();
    assert_eq!(status.stop_reason(), None);
    assert_eq!(status.progress().items, 1);
    crawler
        .queue()
        .push(Seed::get("https://shop.test/shared").unwrap())
        .await
        .unwrap();
    crawler.run().await.unwrap();
    assert_eq!(status.stop_reason(), Some(StopReason::TargetItems));
    assert_eq!(status.progress().items, 2);
}

#[tokio::test]
async fn queued_work_for_a_completed_objective_can_skip_fetching() {
    let site = fixtures::site();
    let router = Router::new()
        .route("plans", AgentCollect::<Plan>::new())
        .route("contacts", AgentCollect::<Contact>::new())
        .with_objective(Objective::<Plan>::new("plans").target_items(1))
        .with_objective(Objective::<Contact>::new("contacts").target_items(2));
    let contacts = router.objective::<Contact>().unwrap();
    let crawler = Crawler::new(site.clone(), router)
        .with_agent(Agent::new(fixtures::provider()))
        .with_dataset(InMemDataset::<Plan>::new())
        .with_dataset(InMemDataset::<Contact>::new())
        .with_concurrency(1)
        .with_seed(
            Seed::get("https://shop.test/pricing")
                .unwrap()
                .with_tag("plans"),
        )
        .with_seed(
            Seed::get("https://shop.test/shared")
                .unwrap()
                .with_tag("plans"),
        )
        .with_seed(
            Seed::get("https://shop.test/contact")
                .unwrap()
                .with_tag("contacts"),
        );
    assert_eq!(crawler.run().await.unwrap().requests, 3);
    assert_eq!(
        site.requests()
            .iter()
            .map(spire::http::http::Uri::path)
            .collect::<Vec<_>>(),
        ["/pricing", "/contact"]
    );
    assert_eq!(contacts.stop_reason(), None);
}

#[tokio::test]
async fn checks_new_evidence_after_handlers_without_waiting_for_exhaustion() {
    let provider = fixtures::provider();
    let router = Router::new()
        .fallback(AgentCollect::<Plan>::new())
        .with_objective(Objective::<Plan>::new("plans").done_when("Basic has a price"));
    let status = router.objective::<Plan>().unwrap();
    let site = fixtures::site();
    let crawler = Crawler::new(site.clone(), router)
        .with_agent(Agent::new(provider.clone()))
        .with_dataset(InMemDataset::<Plan>::new())
        .with_concurrency(1)
        .with_seed(Seed::get("https://shop.test/pricing").unwrap())
        .with_seed(Seed::get("https://shop.test/shared").unwrap());
    assert_eq!(crawler.run().await.unwrap().requests, 1);
    assert_eq!(status.stop_reason(), Some(StopReason::Met));
    assert_eq!(provider.calls(), 2);
}

#[tokio::test]
async fn failed_storage_does_not_accept_records_or_hide_worker_errors() {
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
    let router = Router::new()
        .fallback(AgentCollect::<Plan>::new())
        .with_objective(Objective::<Plan>::new("plans").target_items(1));
    let status = router.objective::<Plan>().unwrap();
    let summary = Crawler::new(fixtures::site(), router)
        .with_agent(Agent::new(fixtures::provider()))
        .with_dataset(Reject)
        .with_seed(Seed::get("https://shop.test/pricing").unwrap())
        .run()
        .await
        .unwrap();
    assert_eq!(summary.failed, 1);
    assert_eq!(status.progress().items, 0);
    assert_eq!(status.stop_reason(), None);
}

#[tokio::test]
async fn invalid_configuration_fails_without_fetching_and_budget_errors_propagate() {
    let site = fixtures::site();
    let router = Router::new()
        .fallback(AgentCollect::<Plan>::new())
        .with_objective(Objective::<Plan>::new("first"))
        .with_objective(Objective::<Plan>::new("second"));
    let error = Crawler::new(site.clone(), router)
        .with_agent(Agent::new(fixtures::provider()))
        .with_seed(Seed::get("https://shop.test/pricing").unwrap())
        .run()
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("duplicate objective record type")
    );
    assert_eq!(site.requests().len(), 0);

    let router = Router::new()
        .fallback(AgentCollect::<Plan>::new())
        .with_objective(Objective::<Plan>::new("plans"));
    let status = router.objective::<Plan>().unwrap();
    let error = Crawler::new(site, router)
        .with_agent(Agent::new(fixtures::provider()).with_budget(Budget::new(0, 0)))
        .with_dataset(InMemDataset::<Plan>::new())
        .with_seed(Seed::get("https://shop.test/pricing").unwrap())
        .run()
        .await
        .unwrap_err();
    assert!(error.to_string().contains("budget exhausted"));
    assert_eq!(status.stop_reason(), None);
}

#[tokio::test]
async fn user_fatal_outcomes_are_preserved_without_extra_judgments() {
    async fn fail() -> spire::worker::Outcome {
        spire::worker::Outcome::Fatal(spire::Error::worker("application failed"))
    }
    let provider = fixtures::provider();
    let router = Router::new()
        .fallback((AgentCollect::<Plan>::new(), fail))
        .with_objective(Objective::<Plan>::new("plans").done_when("Basic has a price"));
    let status = router.objective::<Plan>().unwrap();
    let error = Crawler::new(fixtures::site(), router)
        .with_agent(Agent::new(provider.clone()))
        .with_dataset(InMemDataset::<Plan>::new())
        .with_seed(Seed::get("https://shop.test/pricing").unwrap())
        .run()
        .await
        .unwrap_err();
    assert!(error.to_string().contains("application failed"));
    assert_eq!(status.progress().items, 1);
    assert_eq!(status.stop_reason(), None);
    assert_eq!(provider.calls(), 1);
}

#[tokio::test]
async fn missing_agent_is_a_configuration_error_before_fetching() {
    let site = fixtures::site();
    let router = Router::new()
        .fallback(AgentCollect::<Plan>::new())
        .with_objective(Objective::<Plan>::new("plans"));
    let error = Crawler::new(site.clone(), router)
        .with_seed(Seed::get("https://shop.test/pricing").unwrap())
        .run()
        .await
        .unwrap_err();
    assert!(error.to_string().contains("requires .with_agent"));
    assert_eq!(site.requests().len(), 0);
}

#[tokio::test]
async fn cancellation_does_not_leave_the_completion_check_locked() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use spire::agent::{ProviderResponse, Usage};
    use spire::testing::Script;
    let entered = Arc::new(tokio::sync::Notify::new());
    let judgments = Arc::new(AtomicUsize::new(0));
    let provider = Script::from_async_fn({
        let entered = entered.clone();
        let judgments = judgments.clone();
        move |request| {
            let entered = entered.clone();
            let judgments = judgments.clone();
            async move {
                let text = if request.system.starts_with("Judge whether") {
                    if judgments.fetch_add(1, Ordering::SeqCst) == 0 {
                        entered.notify_one();
                        std::future::pending::<()>().await;
                    }
                    r#"{"met":true}"#
                } else {
                    r#"{"result":[{"name":"Basic","price":"$9"}]}"#
                };
                Ok(ProviderResponse {
                    text: text.into(),
                    usage: Usage::tokens(10, 5),
                })
            }
        }
    });
    let agent = Agent::new(provider);
    let router = Router::new()
        .fallback(AgentCollect::<Plan>::new())
        .with_objective(Objective::<Plan>::new("plans").done_when("Basic has a price"));
    let status = router.objective::<Plan>().unwrap();
    let crawler = Arc::new(
        Crawler::new(fixtures::site(), router)
            .with_agent(agent.clone())
            .with_dataset(InMemDataset::<Plan>::new())
            .with_seed(Seed::get("https://shop.test/pricing").unwrap()),
    );
    let active = crawler.clone();
    let task = tokio::spawn(async move { active.run().await });
    entered.notified().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(status.stop_reason(), None);
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while agent.usage().unmetered_calls == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(agent.usage().unmetered_calls, 1);
    crawler
        .queue()
        .push(Seed::get("https://shop.test/shared").unwrap())
        .await
        .unwrap();
    crawler.run().await.unwrap();
    assert_eq!(status.stop_reason(), Some(StopReason::Met));
    assert_eq!(status.progress().items, 2);
}

#[tokio::test]
async fn follower_queues_canonical_destinations_once_even_without_frontier_deduplication() {
    use spire::testing::{MockPage, MockSite};
    use spire::worker::Outcome;
    let site = MockSite::new().page(
        "https://shop.test/",
        MockPage::html(
            "<a href='/shared'>First</a><a href='/shared?utm_source=test#top'>Again</a>",
        ),
    );
    let router = Router::new()
        .fallback(FollowRelevantLinks::new("page"))
        .with_objective(Objective::<()>::new("discover links").score_with(|_| Some(100)));
    let visit = site
        .visit(Seed::get("https://shop.test/").unwrap())
        .with_extension(Agent::new(fixtures::provider()));
    assert!(matches!(visit.run(&router).await, Outcome::Continue));
    assert_eq!(visit.queued().len(), 1);
    assert_eq!(visit.queued()[0].uri().path(), "/shared");
}

#[tokio::test]
async fn requests_share_progress_through_the_registry_handle() {
    use std::sync::{Arc, Mutex};

    use spire::agent::objective::ObjectiveRegistry;
    use spire::backend::Noop;
    use spire::extract::Extension;
    use spire::http::http::Uri;
    use spire::testing::Script;

    type Observations = Arc<Mutex<Vec<u64>>>;
    async fn handler(
        State(observations): State<Observations>,
        Extension(registry): Extension<ObjectiveRegistry>,
        uri: Uri,
    ) {
        observations
            .lock()
            .unwrap()
            .push(registry.get::<String>().unwrap().progress().items);
        registry
            .prepare(&vec!["accepted".to_owned()], &uri)
            .unwrap()
            .unwrap()
            .accept();
    }
    let observations = Observations::default();
    let router = Router::with_state(observations.clone())
        .fallback(handler)
        .with_objective(Objective::<String>::new("observe requests").target_items(2));
    let progress = router.objective::<String>().unwrap();
    let crawler = Crawler::new(Noop, router)
        .with_agent(Agent::new(Script::default()))
        .with_concurrency(1)
        .with_seed(Seed::get("https://shop.test/one").unwrap())
        .with_seed(Seed::get("https://shop.test/two").unwrap());
    crawler.run().await.unwrap();
    assert_eq!(*observations.lock().unwrap(), [0, 1]);
    assert_eq!(progress.progress().items, 2);
    assert_eq!(progress.stop_reason(), Some(StopReason::TargetItems));
}

#[tokio::test]
async fn follower_skips_canonical_self_links_before_rating_but_keeps_pagination() {
    use spire::agent::{ProviderResponse, Usage};
    use spire::testing::{MockPage, MockSite, Script};
    use spire::worker::Outcome;

    let site = MockSite::new().page(
        "https://shop.test/?utm_medium=seed",
        MockPage::html("<a href='/'>Self</a><a href='/?utm_source=copy'>Tracked self</a><a href='/?fbclid=copy#top'>Another self</a><a href='/#section'>Fragment</a><a href='/?page=2'>Next page</a>"),
    );
    let provider = Script::from_fn(|request| {
        let input: spire::agent::serde_json::Value =
            spire::agent::serde_json::from_str(&request.input).unwrap();
        let links = input["links"].as_array().unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0]["url"], "https://shop.test/?page=2");
        Ok(ProviderResponse {
            text: r#"{"scores":[100]}"#.into(),
            usage: Usage::tokens(10, 5),
        })
    });
    let router = Router::new()
        .fallback(FollowRelevantLinks::new("page"))
        .with_objective(Objective::<()>::new("discover pages"));
    let visit = site
        .visit(Seed::get("https://shop.test/?utm_medium=seed").unwrap())
        .with_extension(Agent::new(provider.clone()));
    assert!(matches!(visit.run(&router).await, Outcome::Continue));
    assert_eq!(provider.calls(), 1);
    assert_eq!(visit.queued().len(), 1);
    assert_eq!(visit.queued()[0].uri(), "https://shop.test/?page=2");
}
