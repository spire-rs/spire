//! Redirect admission, timeouts and graceful shutdown.

use super::*;

#[tokio::test(start_paused = true)]
async fn redirect_targets_wait_for_host_delay_even_when_all_global_slots_are_occupied() {
    use crate::testing::{MockPage, MockSession, MockSite};

    let site = MockSite::new()
        .page(
            "https://example.com/start",
            MockPage::redirect("https://example.com/target"),
        )
        .page("https://example.com/target", MockPage::text("done"));
    let started = Instant::now();
    let worker = from_fn(move |cx: Context<MockSession>| async move {
        assert_eq!(cx.body().await?.as_ref(), b"done");
        assert_eq!(started.elapsed(), Duration::from_secs(2));
        Ok::<_, Error>(())
    });
    let stats = Crawler::new(site, worker)
        .with_concurrency(1)
        .with_host_concurrency(1)
        .with_host_delay(Duration::from_secs(2))
        .with_seed(seed("start"))
        .run()
        .await
        .unwrap();
    assert_eq!((stats.succeeded, stats.failed), (1, 0));
}

#[tokio::test]
async fn redirects_cannot_escape_host_filters_nested_inside_other_facades() {
    use crate::testing::{MockPage, MockSession, MockSite};

    let site = MockSite::new()
        .page(
            "https://example.com/start",
            MockPage::redirect("https://elsewhere.test/secret"),
        )
        .page("https://elsewhere.test/secret", MockPage::text("secret"));
    let worker = from_fn(|cx: Context<MockSession>| async move { cx.body().await.map(drop) });
    let stats = Crawler::new(site, worker)
        .with_frontier(
            InMemFrontier::fifo()
                .allow_hosts(["example.com"])
                .dedup()
                .max_depth(2)
                .retry(3)
                .tag_backoff(),
        )
        .with_seed(seed("start"))
        .run()
        .await
        .unwrap();
    assert_eq!((stats.succeeded, stats.failed, stats.retried), (0, 1, 0));
}

#[cfg(feature = "robots")]
#[tokio::test]
async fn redirects_cannot_bypass_robots_rules_on_the_destination_host() {
    use crate::testing::{MockPage, MockSession, MockSite};

    let site = MockSite::new()
        .page(
            "https://example.com/robots.txt",
            MockPage::text("User-agent: *\nAllow: /\n"),
        )
        .page(
            "https://elsewhere.test/robots.txt",
            MockPage::text("User-agent: *\nDisallow: /private\n"),
        )
        .page(
            "https://example.com/start",
            MockPage::redirect("https://elsewhere.test/private"),
        )
        .page("https://elsewhere.test/private", MockPage::text("secret"));
    let worker = from_fn(|cx: Context<MockSession>| async move { cx.body().await.map(drop) });
    let stats = Crawler::new(site, worker)
        .with_concurrency(1)
        .with_robots("bot")
        .with_seed(seed("start"))
        .run()
        .await
        .unwrap();
    assert_eq!((stats.succeeded, stats.failed), (0, 1));
}

#[tokio::test(start_paused = true)]
async fn redirect_retry_after_pauses_the_destination_without_configured_host_limits() {
    use std::sync::Mutex;

    use ::http::header::RETRY_AFTER;
    use ::http::{HeaderValue, StatusCode};

    use crate::testing::{MockPage, MockSession, MockSite};

    let site = MockSite::new()
        .page(
            "https://example.com/start",
            MockPage::redirect("https://target.test/limited"),
        )
        .page(
            "https://target.test/limited",
            MockPage::status(StatusCode::REQUEST_TIMEOUT)
                .with_header(RETRY_AFTER, HeaderValue::from_static("5")),
        )
        .page("https://example.com/other", MockPage::text(""))
        .page("https://target.test/other", MockPage::text(""));
    let times = Arc::new(Mutex::new(Vec::new()));
    let records = times.clone();
    let started = Instant::now();
    let worker = from_fn(move |cx: Context<MockSession>| {
        records
            .lock()
            .unwrap()
            .push((cx.request().uri().to_string(), started.elapsed()));
        async move { cx.body().await.map(drop) }
    });
    Crawler::new(site, worker)
        .with_concurrency(1)
        .with_frontier(InMemFrontier::fifo())
        .with_seeds([
            seed("start"),
            seed("other"),
            Seed::get("https://target.test/other").unwrap(),
        ])
        .run()
        .await
        .unwrap();
    let times = times.lock().unwrap();
    assert_eq!(
        times[1],
        ("https://example.com/other".into(), Duration::ZERO)
    );
    assert_eq!(
        times[2],
        ("https://target.test/other".into(), Duration::from_secs(5))
    );
}

#[tokio::test]
async fn redirect_admission_follows_contexts_moved_to_a_child_task() {
    use crate::testing::{MockPage, MockSession, MockSite};

    let site = MockSite::new()
        .page(
            "https://example.com/start",
            MockPage::redirect("https://example.com/target"),
        )
        .page("https://example.com/target", MockPage::text("done"));
    let worker = from_fn(|cx: Context<MockSession>| async move {
        tokio::spawn(async move { cx.body().await.map(drop) })
            .await
            .unwrap()
    });
    let stats = Crawler::new(site, worker)
        .with_concurrency(1)
        .with_request_timeout(Duration::from_secs(2))
        .with_seed(seed("start"))
        .run()
        .await
        .unwrap();
    assert_eq!(stats.succeeded, 1);
}

#[tokio::test(start_paused = true)]
async fn request_budget_allows_existing_redirects_to_finish_through_host_delay() {
    use crate::testing::{MockPage, MockSession, MockSite};

    let site = MockSite::new()
        .page(
            "https://example.com/start",
            MockPage::redirect("https://example.com/target"),
        )
        .page("https://example.com/target", MockPage::text("done"));
    let worker = from_fn(|cx: Context<MockSession>| async move { cx.body().await.map(drop) });
    let stats = Crawler::new(site, worker)
        .with_max_requests(1)
        .with_host_concurrency(1)
        .with_host_delay(Duration::from_secs(2))
        .with_seed(seed("start"))
        .run()
        .await
        .unwrap();
    assert_eq!((stats.succeeded, stats.failed, stats.cancelled), (1, 0, 0));
}

#[tokio::test(start_paused = true)]
async fn shutdown_grace_still_cancels_a_redirect_waiting_for_host_delay() {
    use crate::frontier::Frontier;
    use crate::testing::{MockPage, MockSession, MockSite};

    let site = MockSite::new()
        .page(
            "https://example.com/start",
            MockPage::redirect("https://example.com/target"),
        )
        .page("https://example.com/target", MockPage::text("done"));
    let worker = from_fn(|cx: Context<MockSession>| async move { cx.body().await.map(drop) });
    let frontier = Arc::new(InMemFrontier::fifo());
    let stats = Crawler::new(site, worker)
        .with_frontier(frontier.clone())
        .with_max_requests(1)
        .with_shutdown_grace(Duration::from_millis(100))
        .with_host_delay(Duration::from_secs(2))
        .with_seed(seed("start"))
        .run()
        .await
        .unwrap();
    assert_eq!(stats.cancelled, 1);
    assert_eq!(frontier.snapshot().unwrap().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn timed_out_redirects_do_not_keep_the_crawl_waiting_for_host_delay() {
    use crate::testing::{MockPage, MockSession, MockSite};

    let site = MockSite::new()
        .page(
            "https://example.com/start",
            MockPage::redirect("https://example.com/target"),
        )
        .page("https://example.com/target", MockPage::text("done"));
    let worker = from_fn(|cx: Context<MockSession>| async move { cx.body().await.map(drop) });
    let started = Instant::now();
    let stats = Crawler::new(site, worker)
        .with_frontier(InMemFrontier::fifo())
        .with_concurrency(1)
        .with_host_concurrency(1)
        .with_host_delay(Duration::from_secs(3600))
        .with_request_timeout(Duration::from_secs(1))
        .with_seed(seed("start"))
        .run()
        .await
        .unwrap();
    assert_eq!(stats.failed, 1);
    assert_eq!(started.elapsed(), Duration::from_secs(1));
}

#[cfg(feature = "robots")]
#[tokio::test(start_paused = true)]
async fn timed_out_redirects_do_not_wait_for_an_orphaned_robots_fetch() {
    use crate::testing::{MockPage, MockSession, MockSite};

    let site = MockSite::new()
        .page(
            "https://example.com/robots.txt",
            MockPage::text("User-agent: *\nAllow: /\n"),
        )
        .page(
            "https://example.com/start",
            MockPage::redirect("https://target.test/page"),
        )
        .page(
            "https://target.test/robots.txt",
            MockPage::text("User-agent: *\nAllow: /\n").with_latency(Duration::from_secs(3600)),
        )
        .page("https://target.test/page", MockPage::text("done"));
    let worker = from_fn(|cx: Context<MockSession>| async move { cx.body().await.map(drop) });
    let started = Instant::now();
    let stats = Crawler::new(site, worker)
        .with_frontier(InMemFrontier::fifo())
        .with_robots("bot")
        .with_request_timeout(Duration::from_secs(1))
        .with_seed(seed("start"))
        .run()
        .await
        .unwrap();
    assert_eq!(stats.failed, 1);
    assert_eq!(started.elapsed(), Duration::from_secs(1));
}

#[cfg(feature = "robots")]
#[tokio::test(start_paused = true)]
async fn a_parked_redirect_timeout_neither_releases_another_host_slot_nor_keeps_robots_work_alive()
{
    use crate::testing::{MockPage, MockSession, MockSite};

    let site = MockSite::new()
        .page(
            "https://example.com/robots.txt",
            MockPage::text("User-agent: *\nAllow: /\n"),
        )
        .page(
            "https://example.com/start",
            MockPage::redirect("https://target.test/page"),
        )
        .page(
            "https://target.test/robots.txt",
            MockPage::text("User-agent: *\nAllow: /\n").with_latency(Duration::from_secs(3600)),
        )
        .page(
            "https://example.com/a",
            MockPage::text("").with_latency(Duration::from_secs(2)),
        )
        .page(
            "https://example.com/b",
            MockPage::text("").with_latency(Duration::from_secs(2)),
        );
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let observed = peak.clone();
    let worker = from_fn(move |cx: Context<MockSession>| {
        let active = active.clone();
        let peak = peak.clone();
        async move {
            if cx.request().uri().path() == "/start" {
                return tokio::time::timeout(Duration::from_secs(1), cx.body())
                    .await
                    .map_err(|_| Error::new(ErrorKind::Timeout))?
                    .map(drop);
            }
            let count = active.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(count, Ordering::SeqCst);
            let result = cx.body().await.map(drop);
            active.fetch_sub(1, Ordering::SeqCst);
            result
        }
    });
    let started = Instant::now();
    let stats = Crawler::new(site, worker)
        .with_frontier(InMemFrontier::fifo())
        .with_concurrency(3)
        .with_host_concurrency(1)
        .with_robots("bot")
        .with_seeds([seed("start"), seed("a"), seed("b")])
        .run()
        .await
        .unwrap();
    assert_eq!((stats.failed, stats.succeeded), (1, 2));
    assert_eq!(observed.load(Ordering::SeqCst), 1);
    assert_eq!(started.elapsed(), Duration::from_secs(4));
}

#[cfg(feature = "robots")]
#[tokio::test(start_paused = true)]
async fn orphaned_robots_fetches_are_cancelled_while_another_origin_is_still_parked() {
    use crate::testing::{MockPage, MockSession, MockSite};

    let site = MockSite::new()
        .page(
            "https://example.com/robots.txt",
            MockPage::text("User-agent: *\nAllow: /\n"),
        )
        .page(
            "https://example.com/start",
            MockPage::redirect("https://target.test/page"),
        )
        .page(
            "https://target.test/robots.txt",
            MockPage::text("User-agent: *\nAllow: /\n").with_latency(Duration::from_secs(3600)),
        )
        .page(
            "https://other.test/robots.txt",
            MockPage::text("User-agent: *\nDisallow: /\n").with_latency(Duration::from_secs(2)),
        );
    let worker = from_fn(|cx: Context<MockSession>| async move {
        tokio::time::timeout(Duration::from_secs(1), cx.body())
            .await
            .map_err(|_| Error::new(ErrorKind::Timeout))?
            .map(drop)
    });
    let started = Instant::now();
    let stats = Crawler::new(site, worker)
        .with_frontier(InMemFrontier::fifo())
        .with_robots("bot")
        .with_seeds([seed("start"), Seed::get("https://other.test/page").unwrap()])
        .run()
        .await
        .unwrap();
    assert_eq!((stats.failed, stats.skipped), (1, 1));
    assert_eq!(started.elapsed(), Duration::from_secs(2));
}

#[cfg(feature = "robots")]
#[tokio::test(start_paused = true)]
async fn finishing_requests_does_not_cancel_a_legitimate_robots_refresh() {
    use crate::crawler::Robots;
    use crate::testing::{MockPage, MockSession, MockSite};

    let site = MockSite::new().page(
        "https://example.com/robots.txt",
        MockPage::text("User-agent: *\nAllow: /\n").with_latency(Duration::from_secs(1)),
    );
    let worker = from_fn(|cx: Context<MockSession>| async move {
        if cx.request().uri().path() == "/start" {
            cx.queue().follow("/next").await?;
        }
        Ok::<_, Error>(())
    });
    let started = Instant::now();
    let stats = Crawler::new(site, worker)
        .with_frontier(InMemFrontier::fifo())
        .with_robots(Robots::new("bot").with_max_age(Duration::ZERO))
        .with_seed(seed("start"))
        .run()
        .await
        .unwrap();
    assert_eq!(stats.succeeded, 2);
    assert_eq!(started.elapsed(), Duration::from_secs(2));
}

#[tokio::test(start_paused = true)]
async fn slow_sources_do_not_slow_down_fast_redirect_destinations() {
    use futures::StreamExt;

    use crate::crawler::HostConcurrency;
    use crate::crawler::event::Event;
    use crate::testing::{MockPage, MockSession, MockSite};

    let site = MockSite::new()
        .page(
            "https://target.test/fast",
            MockPage::text("").with_latency(Duration::from_millis(10)),
        )
        .page(
            "https://example.com/start",
            MockPage::redirect("https://target.test/fast").with_latency(Duration::from_secs(5)),
        );
    let worker = from_fn(|cx: Context<MockSession>| async move { cx.body().await.map(drop) });
    let crawler = Crawler::new(site, worker)
        .with_frontier(InMemFrontier::fifo())
        .with_concurrency(1)
        .with_host_concurrency(HostConcurrency::adaptive(4))
        .with_seeds([
            Seed::get("https://target.test/fast").unwrap(),
            seed("start"),
        ]);
    let mut events = crawler.events();
    let stats = crawler.run().await.unwrap();
    assert_eq!(stats.succeeded, 2);
    while let Some(event) = events.next().await {
        assert!(!matches!(event, Event::HostSlowed { .. }), "{event:?}");
        if matches!(event, Event::Finished { .. }) {
            break;
        }
    }
}

#[tokio::test(start_paused = true)]
async fn cancelling_a_parked_fetch_does_not_deadlock_a_retry_in_the_same_context() {
    use crate::testing::{MockPage, MockSession, MockSite};

    let site = MockSite::new()
        .page(
            "https://example.com/start",
            MockPage::redirect("https://example.com/target"),
        )
        .page("https://example.com/target", MockPage::text("done"));
    let worker = from_fn(|cx: Context<MockSession>| async move {
        assert!(
            tokio::time::timeout(Duration::from_secs(1), cx.body())
                .await
                .is_err()
        );
        let body = tokio::time::timeout(Duration::from_secs(10), cx.body())
            .await
            .expect("retry must not wait on an abandoned host slot")?;
        assert_eq!(body.as_ref(), b"done");
        Ok::<_, Error>(())
    });
    let stats = Crawler::new(site, worker)
        .with_concurrency(1)
        .with_host_concurrency(1)
        .with_host_delay(Duration::from_secs(2))
        .with_seed(seed("start"))
        .run()
        .await
        .unwrap();
    assert_eq!((stats.succeeded, stats.failed), (1, 0));
}
