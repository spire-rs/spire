use std::time::Duration;

use ::http::header::RETRY_AFTER;
use ::http::{HeaderValue, StatusCode};
use futures::StreamExt;
use tokio::time::sleep;

use super::{Emitter, Event};
use crate::backend::{Backend, Noop};
use crate::crawler::Crawler;
use crate::frontier::facade::RetryPolicy;
use crate::frontier::seed::Seed;
use crate::frontier::{FrontierExt, InMemFrontier};
use crate::testing::{MockPage, MockSession, MockSite};
use crate::worker::{Outcome, Scope, Worker, from_fn};
use crate::{Context, Error};

fn seed(path: &str) -> Seed {
    Seed::get(format!("https://example.com/{path}")).unwrap()
}

/// Names an event briefly, for comparing event orders.
fn label(event: &Event) -> String {
    match event {
        Event::Started => "started".to_owned(),
        Event::RequestStarted { request } => {
            format!("start {} #{}", request.uri.path(), request.attempt.get())
        }
        Event::Fetched {
            request, status, ..
        } => {
            let status = status.map_or("-".to_owned(), |status| status.as_u16().to_string());
            format!("fetch {} {status}", request.uri.path())
        }
        Event::Handled {
            request, outcome, ..
        } => format!("handle {} {outcome}", request.uri.path()),
        Event::Retried { request, .. } => format!("retry {}", request.uri.path()),
        Event::Failed { request } => format!("fail {}", request.uri.path()),
        Event::Skipped { request, reason } => format!("skip {} {reason}", request.uri.path()),
        Event::Cancelled { request } => format!("cancel {}", request.uri.path()),
        Event::HostSlowed { host, .. } => format!("slow {host}"),
        Event::HostPaused { host, duration } => format!("pause {host} {}s", duration.as_secs()),
        Event::RobotsFetched { origin, error } => {
            let result = if error.is_some() { "err" } else { "ok" };
            format!("robots {origin} {result}")
        }
        Event::Stopping { reason } => format!("stop {reason}"),
        Event::Finished { stats } => format!("finish {}", stats.requests),
        Event::Lagged { missed } => format!("lagged {missed}"),
    }
}

/// Runs `crawler`, returning every event of the run.
async fn events<B, W>(crawler: Crawler<B, W>) -> Vec<String>
where
    B: Backend,
    W: Worker<B::Session>,
{
    let mut events = crawler.events();
    let collect = tokio::spawn(async move {
        let mut labels = Vec::new();
        while let Some(event) = events.next().await {
            let finished = matches!(event, Event::Finished { .. });
            labels.push(label(&event));
            if finished {
                break;
            }
        }
        labels
    });
    let _ = crawler.run().await;
    collect.await.unwrap()
}

#[tokio::test(start_paused = true)]
async fn a_small_crawl_reports_events_in_order() {
    let site = MockSite::new()
        .page(
            "https://example.com/",
            MockPage::html("<a href=\"/a\">a</a>"),
        )
        .page("https://example.com/a", MockPage::text("a"))
        .page(
            "https://example.com/flaky",
            MockPage::status(StatusCode::INTERNAL_SERVER_ERROR),
        );
    let worker = from_fn(|cx: Context<MockSession>| async move {
        cx.body().await?;
        if cx.request().uri().path() == "/" {
            cx.queue().follow("/a").await?;
            cx.queue().follow("/flaky").await?;
        }
        Ok::<_, Error>(())
    });
    let policy = RetryPolicy::new(1)
        .with_base_delay(Duration::ZERO)
        .with_jitter(0);
    let crawler = Crawler::new(site, worker)
        .with_frontier(InMemFrontier::fifo().retry_with(policy))
        .with_concurrency(1)
        .with_seed(seed(""));

    assert_eq!(
        events(crawler).await,
        [
            "started",
            "start / #0",
            "fetch / 200",
            "handle / continue",
            "start /a #0",
            "fetch /a 200",
            "handle /a continue",
            "start /flaky #0",
            "fetch /flaky 500",
            "handle /flaky fail",
            "retry /flaky",
            "start /flaky #1",
            "fetch /flaky 500",
            "handle /flaky fail",
            "fail /flaky",
            "finish 4",
        ]
    );
}

#[tokio::test]
async fn aborted_tags_are_skipped() {
    let worker = from_fn(|_: Context<Noop>| async { Outcome::Abort(Scope::SameTag) });
    let crawler = Crawler::new(Noop, worker)
        .with_concurrency(1)
        .with_seeds([seed("1").with_tag("a"), seed("2").with_tag("a")]);

    assert_eq!(
        events(crawler).await,
        [
            "started",
            "start /1 #0",
            "handle /1 abort",
            "skip /2 tag aborted",
            "finish 1",
        ]
    );
}

#[tokio::test]
async fn the_request_budget_stops_the_run() {
    let worker =
        from_fn(|cx: Context<Noop>| async move { cx.queue().follow("/next").await.map(drop) });
    let crawler = Crawler::new(Noop, worker)
        .with_max_requests(1)
        .with_seed(seed("start"));

    assert_eq!(
        events(crawler).await,
        [
            "started",
            "start /start #0",
            // The budget counts requests as they start.
            "stop request budget spent",
            "handle /start continue",
            "finish 1",
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn throttled_hosts_are_slowed_down_or_paused() {
    let limited = MockPage::status(StatusCode::TOO_MANY_REQUESTS)
        .with_header(RETRY_AFTER, HeaderValue::from_static("5"));
    let site = MockSite::new()
        .page("https://example.com/limited", limited)
        .page(
            "https://example.com/busy",
            MockPage::status(StatusCode::SERVICE_UNAVAILABLE),
        );
    let worker = from_fn(|cx: Context<MockSession>| async move { cx.response().await.map(drop) });
    let crawler = Crawler::new(site, worker)
        .with_frontier(InMemFrontier::fifo())
        .with_concurrency(1)
        .with_seeds([seed("limited"), seed("busy")]);

    assert_eq!(
        events(crawler).await,
        [
            "started",
            "start /limited #0",
            "fetch /limited 429",
            "handle /limited fail",
            "pause example.com 5s",
            "fail /limited",
            "start /busy #0",
            "fetch /busy 503",
            "handle /busy fail",
            "slow example.com",
            "fail /busy",
            "finish 2",
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn requests_past_the_grace_period_are_cancelled() {
    let worker = from_fn(|_: Context<Noop>| async {
        sleep(Duration::from_mins(1)).await;
    });
    let crawler = Crawler::new(Noop, worker)
        .with_shutdown_grace(Duration::from_secs(1))
        .with_seed(seed("slow"));
    let shutdown = crawler.shutdown_token();
    tokio::spawn(async move {
        sleep(Duration::from_secs(1)).await;
        shutdown.cancel();
    });

    assert_eq!(
        events(crawler).await,
        [
            "started",
            "start /slow #0",
            "stop shutdown requested",
            "cancel /slow",
            "finish 0",
        ]
    );
}

#[cfg(feature = "robots")]
#[tokio::test(start_paused = true)]
async fn robots_txt_fetches_and_disallows_are_reported() {
    let site = MockSite::new()
        .page(
            "https://example.com/robots.txt",
            MockPage::text("User-agent: *\nDisallow: /private"),
        )
        .page("https://example.com/", MockPage::text(""));
    let worker = from_fn(|cx: Context<MockSession>| async move { cx.response().await.map(drop) });
    let crawler = Crawler::new(site, worker)
        .with_robots("TestBot")
        .with_seeds([seed(""), seed("private")]);

    assert_eq!(
        events(crawler).await,
        [
            "started",
            "robots https://example.com ok",
            "start / #0",
            "skip /private disallowed by robots.txt",
            "fetch / 200",
            "handle / continue",
            "finish 1",
        ]
    );
}

#[tokio::test]
async fn slow_subscribers_miss_the_oldest_events() {
    let emitter = Emitter::new(2);
    let events = emitter.subscribe();
    // Nothing reads the events as they are sent.
    for path in ["1", "2", "3"] {
        emitter.emit_for(Some(&(&seed(path)).into()), |request| {
            Event::RequestStarted { request }
        });
    }

    // The stream ends once the emitter is dropped.
    drop(emitter);
    let labels: Vec<_> = events.map(|event| label(&event)).collect().await;
    assert_eq!(labels, ["lagged 1", "start /2 #0", "start /3 #0"]);
}

#[tokio::test]
async fn events_end_with_their_crawler() {
    let worker = from_fn(|_: Context<Noop>| async {});
    let crawler = Crawler::new(Noop, worker).with_seed(seed(""));
    let events = crawler.events();
    crawler.run().await.unwrap();

    drop(crawler);
    let labels: Vec<_> = events.map(|event| label(&event)).collect().await;
    assert_eq!(
        labels,
        ["started", "start / #0", "handle / continue", "finish 1"]
    );
}

#[test]
fn events_are_built_only_while_someone_subscribes() {
    let emitter = Emitter::new(8);
    let mut built = 0;
    emitter.emit(|| {
        built += 1;
        Event::Started
    });
    assert_eq!(built, 0);

    let events = emitter.subscribe();
    assert!(emitter.is_active());
    emitter.emit(|| {
        built += 1;
        Event::Started
    });
    assert_eq!(built, 1);

    drop(events);
    assert!(!emitter.is_active());
}
