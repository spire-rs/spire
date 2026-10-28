use std::sync::Arc;

use serde_json::{Value, json};
use spire_core::backend::Noop;
use spire_core::crawler::Crawler;
use spire_core::frontier::score::Keywords;
use spire_core::frontier::seed::Seed;
use spire_core::frontier::{Admission, Frontier, InMemFrontier, NextSeed, Report};
use spire_core::worker::{Worker, from_fn};

use super::run::Phase;
use super::{AgentFrontierExt, Candidate, Finished, Objective, ObjectiveRun, Until, lock};
use crate::cache::InMemCache;
use crate::testing::Script;
use crate::{Agent, Budget, CompletionRequest, CompletionResponse, Error, Usage};

fn script(answer: impl Fn(&CompletionRequest) -> String + Send + Sync + 'static) -> Script {
    Script::from_fn(move |request| {
        Ok(CompletionResponse {
            text: answer(request),
            usage: Usage::tokens(10, 5).with_cost(0.01),
        })
    })
}

fn rating() -> Script {
    script(|request| {
        let input: Value = serde_json::from_str(&request.input).unwrap();
        json!({"scores":vec![90; input["links"].as_array().unwrap().len()]}).to_string()
    })
}

fn candidate(id: usize) -> Candidate {
    Candidate {
        url: format!("https://site.test/page/{id}"),
        text: format!("Page {id}"),
    }
}
fn seed(id: usize) -> Seed {
    Seed::get(candidate(id).url).unwrap()
}

fn crawler(run: &ObjectiveRun) -> Crawler<Noop, impl Worker<Noop>> {
    // End any manually driven scheduling phase before testing the controller.
    lock(&run.shared.state).phase = Phase::Fresh;
    Crawler::new(Noop, from_fn(|_: spire_core::Context<Noop>| async {}))
        .with_frontier(InMemFrontier::fifo().until(run))
}

// These tests drive scheduling manually to isolate the facade's accounting.
// Complete crawl lifecycle tests below and in the facade use ObjectiveRun::run.
fn scheduled(run: &ObjectiveRun) -> Until<InMemFrontier> {
    lock(&run.shared.state).phase = Phase::Running;
    InMemFrontier::fifo().until(run)
}

#[tokio::test]
async fn a_thousand_pages_share_twenty_calls_for_five_thousand_unique_links() {
    let provider = rating();
    let run = Objective::new("plans").start(Agent::new(provider.clone()));
    let mut tasks = Vec::new();
    for page in 0..1000 {
        let run = run.clone();
        tasks.push(tokio::spawn(async move {
            let links = (0..50)
                .map(|link| candidate((page % 100) * 50 + link))
                .collect::<Vec<_>>();
            assert_eq!(run.rate(&links).await.unwrap(), vec![90; 50]);
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    assert_eq!(provider.calls(), 20);
    assert_eq!(run.agent().usage().calls, 20);
}

#[tokio::test(start_paused = true)]
async fn partial_batches_flush_and_cheap_scores_avoid_model_calls() {
    let provider = rating();
    let run = Objective::new("pricing")
        .score_with({
            let keywords = Keywords::new(["pricing"]);
            move |candidate: &Candidate| {
                if candidate.text == "Skip" {
                    Some(-10)
                } else {
                    (keywords.matches(&candidate.text) > 0).then_some(120)
                }
            }
        })
        .start(Agent::new(provider.clone()));
    let pricing = Candidate {
        url: "https://site.test/pricing".into(),
        text: "Pricing".into(),
    };
    assert_eq!(run.rate(&[pricing]).await.unwrap(), [100]);
    assert_eq!(provider.calls(), 0);
    assert_eq!(run.rate(&[candidate(0)]).await.unwrap(), [90]);
    assert_eq!(provider.calls(), 1);
    let rejected = Candidate {
        url: "https://site.test/skip".into(),
        text: "Skip".into(),
    };
    assert_eq!(run.rate(&[rejected]).await.unwrap(), [0]);
    assert_eq!(provider.calls(), 1);
}

#[tokio::test]
async fn canonical_links_are_rated_once_and_bad_answers_are_not_cached() {
    let provider = rating();
    let run = Objective::new("plans").start(Agent::new(provider.clone()));
    let mut duplicate = candidate(0);
    duplicate.url.push_str("?utm_source=test#fragment");
    assert_eq!(
        run.rate(&[candidate(0), duplicate]).await.unwrap(),
        [90, 90]
    );
    assert_eq!(provider.calls(), 1);
    for text in [
        r#"{"scores":[]}"#,
        r#"{"scores":[101]}"#,
        r#"{"scores":[1,2]}"#,
        r#"{"scores":["yes"]}"#,
    ] {
        let provider = script(move |_| text.into());
        let run = Objective::new("plans")
            .start(Agent::new(provider.clone()).with_cache(InMemCache::default(), "bad"));
        for _ in 0..2 {
            assert!(matches!(
                run.rate(&[candidate(0)]).await,
                Err(Error::Answer(_))
            ));
        }
        assert_eq!(provider.calls(), 2);
        assert_eq!(run.agent().usage().cache_hits, 0);
    }
}

#[tokio::test]
async fn extraction_has_no_implicit_progress_and_reports_own_counting_semantics() {
    let provider = script(|_| r#"{"result":["Basic","Pro"]}"#.into());
    let run = Objective::new("plans")
        .target_items(6)
        .start(Agent::new(provider.clone()).with_cache(InMemCache::default(), "plans"));
    for _ in 0..2 {
        let _: Vec<String> = run.agent().extract("Plans").await.unwrap();
    }
    assert_eq!(run.progress().items, 0);
    assert_eq!(provider.calls(), 1);
    run.record_batch(["Basic", "Pro"]).unwrap();
    run.record_batch(["Basic", "Pro"]).unwrap();
    assert_eq!(run.progress().items, 4);
    run.record(vec!["Basic", "Pro"]).unwrap();
    assert_eq!(
        run.progress().items,
        5,
        "an explicitly reported array is one item"
    );
    run.found(1);
    assert_eq!(run.finished(), Some(Finished::TargetItems));
}

#[test]
fn cloned_configurations_start_independent_runtime_state() {
    let config = Objective::new("plans").target_items(2);
    let first = config.clone().start(Agent::new(rating()));
    let second = config.target_items(1).start(Agent::new(rating()));
    first.found(1);
    second.found(1);
    assert_eq!(first.finished(), None);
    assert_eq!(second.finished(), Some(Finished::TargetItems));
    first.clone().found(1);
    assert_eq!(first.finished(), Some(Finished::TargetItems));
}

#[tokio::test]
async fn frontier_tracks_outstanding_work_but_never_calls_a_model() {
    let provider = script(|_| r#"{"met":false}"#.into());
    let run = Objective::new("plans")
        .done_when("prices known")
        .start(Agent::new(provider.clone()));
    let frontier = scheduled(&run);
    frontier.push(seed(0)).await.unwrap();
    let NextSeed::Ready(first) = frontier.pop().await.unwrap() else {
        panic!()
    };
    assert!(matches!(frontier.pop().await.unwrap(), NextSeed::Empty));
    assert!(!lock(&run.shared.state).exhausted);
    assert_eq!(frontier.push(seed(1)).await.unwrap(), Admission::Queued);
    frontier.report(Report::Done(first)).await.unwrap();
    let NextSeed::Ready(second) = frontier.pop().await.unwrap() else {
        panic!()
    };
    frontier.report(Report::Done(second)).await.unwrap();
    run.record("Basic").unwrap();
    assert!(matches!(frontier.pop().await.unwrap(), NextSeed::Empty));
    assert!(lock(&run.shared.state).exhausted);
    assert_eq!(run.finished(), None);
    assert_eq!(provider.calls(), 0);
    run.run(&crawler(&run)).await.unwrap();
    assert_eq!(run.finished(), Some(Finished::Exhausted));
    assert_eq!(provider.calls(), 1);
    assert_eq!(frontier.push(seed(2)).await.unwrap(), Admission::Stopped);
}

#[tokio::test]
async fn page_targets_stalls_and_budget_limits_have_distinct_reasons() {
    let run = Objective::new("plans")
        .target_pages(1)
        .start(Agent::new(rating()));
    run.rate(&[candidate(0)]).await.unwrap();
    let frontier = scheduled(&run);
    frontier.push(seed(0)).await.unwrap();
    let NextSeed::Ready(page) = frontier.pop().await.unwrap() else {
        panic!()
    };
    frontier.report(Report::Done(page)).await.unwrap();
    assert_eq!(run.finished(), Some(Finished::TargetPages));

    let run = Objective::new("plans")
        .stall_after(2)
        .start(Agent::new(rating()));
    let frontier = scheduled(&run);
    for id in 0..3 {
        frontier.push(seed(id)).await.unwrap();
    }
    for _ in 0..2 {
        let NextSeed::Ready(page) = frontier.pop().await.unwrap() else {
            panic!()
        };
        frontier.report(Report::Done(page)).await.unwrap();
    }
    assert_eq!(run.finished(), Some(Finished::Stalled));
    assert_eq!(frontier.snapshot().unwrap().len(), 1);

    let provider = rating();
    let run =
        Objective::new("plans").start(Agent::new(provider.clone()).with_budget(Budget::tokens(0)));
    assert!(matches!(
        run.rate(&[candidate(0)]).await,
        Err(Error::Budget(_))
    ));
    assert_eq!(run.finished(), Some(Finished::Budget));
    assert_eq!(provider.calls(), 0);
}

#[tokio::test]
async fn runtime_owns_final_checks_and_rejects_missing_wiring_or_reuse() {
    let provider = script(|_| r#"{"met":true}"#.into());
    let run = Objective::new("plans")
        .done_when("Basic has a price")
        .check_every(5)
        .start(Agent::new(provider.clone()));
    run.record(json!({"name":"Basic","price":"$9"})).unwrap();
    run.check().await.unwrap();
    assert_eq!(provider.calls(), 0);
    let bare = Crawler::new(Noop, from_fn(|_: spire_core::Context<Noop>| async {}));
    assert!(run.run(&bare).await.is_err());
    let crawler = crawler(&run);
    assert!(
        crawler
            .run()
            .await
            .unwrap_err()
            .to_string()
            .contains("ObjectiveRun::run")
    );
    run.run(&crawler).await.unwrap();
    assert_eq!(provider.calls(), 1);
    assert_eq!(run.finished(), Some(Finished::Met));
    assert!(run.run(&crawler).await.is_err());
}

#[tokio::test(start_paused = true)]
async fn cancelling_a_partial_batch_before_dispatch_does_not_spend_model_usage() {
    let provider = rating();
    let run = Objective::new("plans")
        .rating_delay(std::time::Duration::from_secs(1))
        .start(Agent::new(provider.clone()));
    let task = {
        let run = run.clone();
        tokio::spawn(async move { run.rate(&[candidate(0)]).await })
    };
    tokio::task::yield_now().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::advance(std::time::Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    assert_eq!(provider.calls(), 0);
    assert_eq!(run.rate(&[candidate(0)]).await.unwrap(), [90]);
    assert_eq!(provider.calls(), 1);
}

#[tokio::test]
async fn completion_requires_changed_evidence_and_serializes_concurrent_checks() {
    let provider = script(|_| r#"{"met":false}"#.into());
    let run = Objective::new("plans")
        .done_when("all prices known")
        .check_every(1)
        .start(Agent::new(provider.clone()));
    let frontier = scheduled(&run);
    run.record(json!({"name":"Basic"})).unwrap();
    for id in 0..3 {
        frontier.push(seed(id)).await.unwrap();
        let NextSeed::Ready(page) = frontier.pop().await.unwrap() else {
            panic!()
        };
        frontier.report(Report::Done(page)).await.unwrap();
        let (first, second) = tokio::join!(run.check(), run.check());
        first.unwrap();
        second.unwrap();
    }
    assert_eq!(provider.calls(), 1);
    run.record(json!({"name":"Pro"})).unwrap();
    run.check().await.unwrap();
    assert_eq!(provider.calls(), 2);
}

#[tokio::test]
async fn explicit_records_are_bounded_and_zero_targets_start_no_requests() {
    let provider = rating();
    let run = Objective::new("plans")
        .target_items(150)
        .start(Agent::new(provider.clone()));
    for id in 0..200 {
        run.record(json!({"id":id})).unwrap();
    }
    assert_eq!(run.progress().items, 200);
    assert_eq!(run.progress().record.len(), 128);
    assert_eq!(run.progress().omitted, 72);
    run.run(&crawler(&run).with_seed(seed(0))).await.unwrap();
    assert_eq!(provider.calls(), 0);
    assert_eq!(run.finished(), Some(Finished::TargetItems));
}

struct WaitingJudge {
    script: Script,
    entered: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
}
impl WaitingJudge {
    fn new() -> Self {
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let script = Script::from_async_fn({
            let entered = entered.clone();
            let release = release.clone();
            move |_| {
                let first = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0;
                let entered = entered.clone();
                let release = release.clone();
                async move {
                    if first {
                        entered.notify_one();
                        release.notified().await;
                    }
                    Ok(CompletionResponse {
                        text: r#"{"met":true}"#.into(),
                        usage: Usage::tokens(10, 5).with_cost(0.0),
                    })
                }
            }
        });
        Self {
            script,
            entered,
            release,
        }
    }
}

#[tokio::test]
async fn stale_positive_judgments_cannot_finish_newer_progress() {
    let provider = WaitingJudge::new();
    let run = Objective::new("plans")
        .done_when("all plans have prices")
        .check_every(1)
        .start(Agent::new(provider.script.clone()));
    let frontier = scheduled(&run);
    run.record(json!({"name":"Basic","price":"$9"})).unwrap();
    frontier.push(seed(0)).await.unwrap();
    let NextSeed::Ready(page) = frontier.pop().await.unwrap() else {
        panic!()
    };
    frontier.report(Report::Done(page)).await.unwrap();
    let task = {
        let run = run.clone();
        tokio::spawn(async move { run.check().await })
    };
    provider.entered.notified().await;
    run.record(json!({"name":"Pro","price":null})).unwrap();
    provider.release.notify_one();
    task.await.unwrap().unwrap();
    assert_eq!(run.finished(), None);
    run.run(&crawler(&run)).await.unwrap();
    assert_eq!(run.finished(), Some(Finished::Met));
    assert_eq!(provider.script.calls(), 2);
}

#[tokio::test]
async fn external_shutdown_cancels_final_judgment_without_claiming_exhaustion() {
    let provider = WaitingJudge::new();
    let run = Objective::new("plans")
        .done_when("all prices known")
        .start(Agent::new(provider.script.clone()));
    run.record("Basic").unwrap();
    let crawler = crawler(&run);
    let token = crawler.shutdown_token();
    let task = {
        let run = run.clone();
        tokio::spawn(async move { run.run(&crawler).await })
    };
    provider.entered.notified().await;
    token.cancel();
    task.await.unwrap().unwrap();
    assert_eq!(run.finished(), None);
    assert_eq!(run.agent().usage().unmetered_calls, 1);
}
