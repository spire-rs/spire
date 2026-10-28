//! Provider-independent accounting, caching and structured extraction.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::Deserialize;
use spire_agent::cache::{Cache, InMemCache};
use spire_agent::testing::Script;
use spire_agent::{Agent, Budget, Error, ProviderRequest, ProviderResponse, Usage};
use tokio::sync::Notify;

fn script(answers: &[(&str, Usage)]) -> Script {
    Script::new(answers.iter().map(|(text, usage)| ProviderResponse {
        text: (*text).into(),
        usage: *usage,
    }))
}

#[derive(Debug, Deserialize, JsonSchema, PartialEq)]
struct Plan {
    name: String,
    price: String,
}

fn request(input: &str) -> ProviderRequest {
    ProviderRequest {
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
            Usage::tokens(10, 5),
        ),
        (r#"{"result":[]}"#, Usage::tokens(4, 2)),
        (r#"{"result":null}"#, Usage::tokens(3, 1)),
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
    let requests = provider.requests();
    assert!(requests[0].system.contains("untrusted data"));
    assert!(requests[0].schema.as_ref().unwrap()["properties"]["result"].is_object());
}

#[tokio::test]
async fn input_and_output_caps_independently_stop_new_calls() {
    for (budget, reason) in [
        (Budget::default().input_tokens(3), "input tokens"),
        (Budget::default().output_tokens(2), "output tokens"),
        (Budget::new(3, 20), "input tokens"),
        (Budget::new(30, 2), "output tokens"),
    ] {
        let provider = script(&[("answer", Usage::tokens(3, 2))]);
        let agent = Agent::new(provider.clone()).with_budget(budget);
        agent.complete(request("one")).await.unwrap();
        assert!(
            matches!(agent.complete(request("two")).await, Err(Error::Budget(found)) if found == reason)
        );
        assert_eq!(provider.calls(), 1);
    }
    for budget in [
        Budget::default().input_tokens(0),
        Budget::default().output_tokens(0),
    ] {
        let provider = script(&[]);
        assert!(matches!(
            Agent::new(provider.clone())
                .with_budget(budget)
                .complete(request("x"))
                .await,
            Err(Error::Budget(_))
        ));
        assert_eq!(provider.calls(), 0);
    }
}

#[tokio::test]
async fn only_usage_required_by_a_capped_direction_must_be_known() {
    let missing_input = Usage {
        input_tokens: None,
        output_tokens: Some(1),
    };
    let missing_output = Usage {
        input_tokens: Some(2),
        output_tokens: None,
    };
    for (usage, budget) in [
        (missing_input, Budget::default().input_tokens(10)),
        (missing_output, Budget::default().output_tokens(10)),
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
    }
    for (usage, budget) in [
        (missing_input, Budget::default().output_tokens(10)),
        (missing_output, Budget::default().input_tokens(10)),
    ] {
        let provider = script(&[("one", usage), ("two", usage)]);
        let agent = Agent::new(provider.clone()).with_budget(budget);
        agent.complete(request("one")).await.unwrap();
        agent.complete(request("two")).await.unwrap();
        assert_eq!(provider.calls(), 2);
    }

    // Changing a clone's budget cannot make previously unknown usage known.
    let provider = script(&[("one", missing_input), ("two", missing_input)]);
    let agent = Agent::new(provider.clone());
    agent.complete(request("one")).await.unwrap();
    assert!(matches!(
        agent
            .clone()
            .with_budget(Budget::default().input_tokens(10))
            .complete(request("two"))
            .await,
        Err(Error::Configuration(_))
    ));
    agent
        .with_budget(Budget::default().output_tokens(10))
        .complete(request("two"))
        .await
        .unwrap();
    assert_eq!(provider.calls(), 2);
}

#[tokio::test]
async fn rejects_zero_per_call_output_limits() {
    let provider = script(&[]);
    let agent = Agent::new(provider.clone()).with_max_tokens(0);
    assert!(agent.extract::<String>("x").await.is_err());
    assert_eq!(provider.calls(), 0);
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
        ("not JSON", Usage::tokens(2, 1)),
        (
            r#"{"result":[{"name":2,"price":"9"}]}"#,
            Usage::tokens(2, 1),
        ),
        (r#"{"result":[]}"#, Usage::tokens(2, 1)),
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
            Ok(ProviderResponse {
                text: "answer".into(),
                usage: Usage::tokens(2, 1),
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
            .with_budget(Budget::new(2, 1));
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
        .with_budget(Budget::new(100, 100));
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
    assert_eq!((agent.usage().calls, agent.usage().unmetered_calls), (1, 1));
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
