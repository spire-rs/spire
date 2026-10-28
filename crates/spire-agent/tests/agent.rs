//! Provider-independent accounting, caching and structured extraction.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use spire_agent::cache::{Cache, InMemCache};
use spire_agent::testing::Script;
use spire_agent::{Agent, Budget, CompletionRequest, CompletionResponse, Error, Pricing, Usage};
use tokio::sync::Notify;

fn script(answers: &[(&str, Usage)]) -> Script {
    Script::new(answers.iter().map(|(text, usage)| CompletionResponse {
        text: (*text).into(),
        usage: *usage,
    }))
}

#[derive(Debug, Deserialize, JsonSchema, PartialEq)]
struct Plan {
    name: String,
    price: String,
}

fn request(input: &str) -> CompletionRequest {
    CompletionRequest {
        system: "answer a question".into(),
        input: input.into(),
        schema: None,
        max_tokens: 100,
    }
}

#[tokio::test]
async fn validates_extraction_empty_results_and_shared_usage() {
    let provider = script(&[
        (
            r#"{"result":[{"name":"Basic","price":"$9"}]}"#,
            Usage::tokens(10, 5).with_cost(0.2),
        ),
        (r#"{"result":[]}"#, Usage::tokens(4, 2).with_cost(0.1)),
        (r#"{"result":null}"#, Usage::tokens(3, 1).with_cost(0.01)),
    ]);
    let agent = Agent::new(provider.clone());
    let plans: Vec<Plan> = agent.clone().extract("# Basic\n$9").await.unwrap();
    assert_eq!(
        plans,
        [Plan {
            name: "Basic".into(),
            price: "$9".into()
        }]
    );
    assert_eq!(
        agent
            .extract::<Vec<Plan>>("generated filler")
            .await
            .unwrap(),
        Vec::<Plan>::new()
    );
    assert!(
        agent
            .extract::<Option<Plan>>("nothing here")
            .await
            .unwrap()
            .is_none()
    );
    let totals = agent.usage();
    assert_eq!(
        (totals.calls, totals.input_tokens, totals.output_tokens),
        (3, 17, 8)
    );
    assert!((totals.cost_usd - 0.31).abs() < 1e-12);
    let requests = provider.requests();
    assert!(requests[0].system.contains("untrusted data"));
    assert!(requests[0].schema.as_ref().unwrap()["properties"]["result"].is_object());
}

#[tokio::test]
async fn token_and_cost_limits_stop_new_calls() {
    for budget in [Budget::tokens(5), Budget::default().max_cost(0.5)] {
        let provider = script(&[("answer", Usage::tokens(3, 2).with_cost(0.5))]);
        let agent = Agent::new(provider.clone()).with_budget(budget);
        agent.complete(request("one")).await.unwrap();
        assert!(matches!(
            agent.complete(request("two")).await,
            Err(Error::Budget(_))
        ));
        assert_eq!(provider.calls(), 1);
    }
    let provider = script(&[]);
    assert!(matches!(
        Agent::new(provider.clone())
            .with_budget(Budget::tokens(0))
            .complete(request("x"))
            .await,
        Err(Error::Budget(_))
    ));
    assert_eq!(provider.calls(), 0);
}

#[tokio::test]
async fn prices_fallbacks_but_provider_cost_wins_and_unknown_usage_fails_closed() {
    let provider = script(&[
        ("one", Usage::tokens(1_000_000, 500_000)),
        ("two", Usage::tokens(1, 1).with_cost(0.25)),
    ]);
    let agent = Agent::new(provider).with_pricing(Pricing::new(2.0, 4.0).unwrap());
    agent.complete(request("one")).await.unwrap();
    agent.complete(request("two")).await.unwrap();
    assert!((agent.usage().cost_usd - 4.25).abs() < 1e-12);
    for (usage, budget) in [
        (Usage::tokens(2, 1), Budget::default().max_cost(1.0)),
        (Usage::default().with_cost(0.1), Budget::tokens(10)),
    ] {
        let provider = script(&[("one", usage)]);
        let agent = Agent::new(provider.clone()).with_budget(budget);
        assert!(matches!(
            agent.complete(request("one")).await,
            Err(Error::Configuration(_))
        ));
        assert!(matches!(
            agent.complete(request("two")).await,
            Err(Error::Configuration(_))
        ));
        assert_eq!(provider.calls(), 1);
        assert_eq!(agent.usage().calls, 1);
    }
}

#[tokio::test]
async fn rejects_invalid_limits_and_provider_costs() {
    assert!(Pricing::new(-1.0, 0.0).is_err());
    assert!(Pricing::new(f64::NAN, 1.0).is_err());
    let provider = script(&[]);
    let agent = Agent::new(provider.clone()).with_budget(Budget::default().max_cost(f64::INFINITY));
    assert!(agent.complete(request("x")).await.is_err());
    assert_eq!(provider.calls(), 0);
    let provider = script(&[("x", Usage::tokens(1, 1).with_cost(f64::NAN))]);
    let agent = Agent::new(provider);
    assert!(matches!(
        agent.complete(request("x")).await,
        Err(Error::Configuration(_))
    ));
    assert!(agent.usage().cost_usd.is_finite());
}

#[tokio::test]
async fn cache_keys_include_content_question_schema_limit_and_namespace() {
    let provider = script(&[
        ("a", Usage::tokens(2, 1)),
        ("b", Usage::tokens(2, 1)),
        ("c", Usage::tokens(2, 1)),
        ("d", Usage::tokens(2, 1)),
    ]);
    let agent = Agent::new(provider.clone()).with_cache(InMemCache::default(), "script/model-v1");
    agent.complete(request("page-v1")).await.unwrap();
    agent.clone().complete(request("page-v1")).await.unwrap();
    agent.complete(request("page-v2")).await.unwrap();
    let mut changed = request("page-v1");
    changed.system = "different question".into();
    agent.complete(changed).await.unwrap();
    let mut changed = request("page-v1");
    changed.max_tokens += 1;
    agent.complete(changed).await.unwrap();
    assert_eq!((provider.calls(), agent.usage().cache_hits), (4, 1));
    let cache = InMemCache::new(1);
    cache.put("one", "1").await.unwrap();
    cache.put("two", "2").await.unwrap();
    assert!(cache.get("one").await.unwrap().is_none());
    assert_eq!(cache.get("two").await.unwrap().as_deref(), Some("2"));
}

#[tokio::test]
async fn malformed_or_wrong_schema_answers_are_charged_and_never_cached() {
    let provider = script(&[
        ("not JSON", Usage::tokens(2, 1).with_cost(0.1)),
        (
            r#"{"result":[{"name":2,"price":"9"}]}"#,
            Usage::tokens(2, 1).with_cost(0.1),
        ),
        (r#"{"result":[]}"#, Usage::tokens(2, 1).with_cost(0.1)),
    ]);
    let agent = Agent::new(provider.clone()).with_cache(InMemCache::default(), "model-v1");
    for _ in 0..2 {
        assert!(matches!(
            agent.extract::<Vec<Plan>>("same page").await,
            Err(Error::Answer(_))
        ));
    }
    assert_eq!(
        agent.extract::<Vec<Plan>>("same page").await.unwrap(),
        Vec::<Plan>::new()
    );
    assert_eq!(
        agent.extract::<Vec<Plan>>("same page").await.unwrap(),
        Vec::<Plan>::new()
    );
    assert_eq!(
        (
            provider.calls(),
            agent.usage().cache_hits,
            agent.usage().input_tokens
        ),
        (3, 1, 6)
    );
}

fn waiting(entered: Arc<Notify>, release: Arc<Notify>) -> Script {
    Script::from_async_fn(move |_| {
        let entered = entered.clone();
        let release = release.clone();
        async move {
            entered.notify_one();
            release.notified().await;
            Ok(CompletionResponse {
                text: "answer".into(),
                usage: Usage::tokens(2, 1).with_cost(0.1),
            })
        }
    })
}

#[tokio::test]
async fn queued_calls_recheck_budgets_and_identical_requests_have_single_flight() {
    for cached in [false, true] {
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let provider = waiting(entered.clone(), release.clone());
        let mut agent = Agent::new(provider.clone())
            .with_concurrency(1)
            .with_budget(Budget::tokens(3));
        if cached {
            agent = agent.with_cache(InMemCache::default(), "model");
        }
        let first = {
            let agent = agent.clone();
            tokio::spawn(async move { agent.complete(request("same")).await })
        };
        entered.notified().await;
        let second = {
            let agent = agent.clone();
            tokio::spawn(async move { agent.complete(request("same")).await })
        };
        tokio::task::yield_now().await;
        assert_eq!(provider.calls(), 1);
        release.notify_one();
        first.await.unwrap().unwrap();
        let second = second.await.unwrap();
        assert_eq!(second.is_ok(), cached);
        assert_eq!(provider.calls(), 1);
    }
}

#[tokio::test]
async fn cancellation_releases_gate_and_marks_budget_accounting_unknown() {
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let provider = waiting(entered.clone(), release);
    let agent = Agent::new(provider.clone())
        .with_concurrency(1)
        .with_budget(Budget::tokens(100));
    let pending = {
        let agent = agent.clone();
        tokio::spawn(async move { agent.complete(request("page")).await })
    };
    entered.notified().await;
    pending.abort();
    let _ = pending.await;
    assert!(matches!(
        agent.complete(request("page")).await,
        Err(Error::Configuration(_))
    ));
    assert_eq!(
        (
            agent.usage().calls,
            agent.usage().unmetered_calls,
            agent.usage().unpriced_calls
        ),
        (1, 1, 1)
    );
    assert_eq!(provider.calls(), 1);
}

#[cfg(feature = "redb")]
#[tokio::test]
async fn persistent_answers_survive_reopening() {
    use spire_agent::cache::AgentStoreExt;
    use spire_redb::RedbStore;
    let path = std::env::temp_dir().join(format!(
        "spire-agent-{}-{}.redb",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    {
        let store = RedbStore::open(&path).await.unwrap();
        let cache = store.agent_cache();
        cache.put("key", "answer").await.unwrap();
    }
    {
        let store = RedbStore::open(&path).await.unwrap();
        let cache = store.agent_cache();
        assert_eq!(cache.get("key").await.unwrap().as_deref(), Some("answer"));
    }
    std::fs::remove_file(path).unwrap();
}
