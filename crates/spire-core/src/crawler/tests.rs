use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::time::{Instant, sleep};

use crate::backend::Noop;
use crate::dataset::InMemDataset;
use crate::frontier::FrontierExt;
use crate::frontier::facade::RetryPolicy;
use crate::frontier::memory::InMemFrontier;
use crate::frontier::seed::Seed;
use crate::request::RequestExt;
use crate::worker::{Outcome, from_fn};
use crate::{Context, Crawler, Error, ErrorKind};

fn seed(path: &str) -> Seed {
    Seed::get(format!("https://example.com/{path}")).unwrap()
}

fn path(cx: &Context<Noop>) -> String {
    cx.request().uri().path().trim_start_matches('/').to_owned()
}

#[tokio::test(start_paused = true)]
async fn runs_requests_concurrently() {
    let worker = from_fn(|_: Context<Noop>| async {
        sleep(Duration::from_millis(100)).await;
    });
    let crawler = Crawler::new(Noop, worker)
        .with_concurrency(8)
        .with_seeds((0..8).map(|i| seed(&i.to_string())));

    let started = Instant::now();
    let stats = crawler.run().await.unwrap();
    assert_eq!(stats.requests, 8);
    assert_eq!(started.elapsed(), Duration::from_millis(100));
}

#[tokio::test(start_paused = true)]
async fn concurrency_limit_is_respected() {
    let worker = from_fn(|_: Context<Noop>| async {
        sleep(Duration::from_millis(100)).await;
    });
    let crawler = Crawler::new(Noop, worker)
        .with_concurrency(2)
        .with_seeds((0..6).map(|i| seed(&i.to_string())));

    let started = Instant::now();
    crawler.run().await.unwrap();
    assert_eq!(started.elapsed(), Duration::from_millis(300));
}

#[tokio::test(start_paused = true)]
async fn follows_links_queued_while_others_are_in_flight() {
    // A chain of pages, each found only after its parent was slowly handled.
    let worker = from_fn(|cx: Context<Noop>| async move {
        sleep(Duration::from_millis(10)).await;
        if cx.request().depth().get() < 5 {
            cx.queue().follow("https://example.com/next").await?;
        }
        Ok::<_, Error>(())
    });
    let crawler = Crawler::new(Noop, worker)
        .with_concurrency(4)
        .with_seed(seed("start"));

    let stats = crawler.run().await.unwrap();
    assert_eq!(stats.requests, 6);
    assert_eq!(stats.succeeded, 6);
}

#[tokio::test]
async fn frontier_facades_apply() {
    // Every page links to the same two pages, which link back to the start.
    let worker = from_fn(|cx: Context<Noop>| async move {
        for path in ["a", "b", ""] {
            cx.queue()
                .follow(format!("https://example.com/{path}"))
                .await?;
        }
        Ok::<_, Error>(())
    });
    let crawler = Crawler::new(Noop, worker)
        .with_frontier(InMemFrontier::fifo().dedup().max_depth(1))
        .with_seed(seed(""));

    let stats = crawler.run().await.unwrap();
    assert_eq!(stats.requests, 3);
}

#[tokio::test(start_paused = true)]
async fn retries_until_success() {
    let worker = from_fn(|cx: Context<Noop>| async move {
        if cx.request().attempt().get() < 2 {
            Outcome::retry()
        } else {
            Outcome::Continue
        }
    });
    let policy = RetryPolicy::new(5)
        .with_base_delay(Duration::from_secs(1))
        .with_jitter(0);
    let crawler = Crawler::new(Noop, worker)
        .with_frontier(InMemFrontier::fifo().retry_with(policy))
        .with_seed(seed("flaky"));

    let started = Instant::now();
    let stats = crawler.run().await.unwrap();
    assert_eq!((stats.requests, stats.retried, stats.succeeded), (3, 2, 1));
    assert_eq!(started.elapsed(), Duration::from_secs(1 + 2));
}

#[tokio::test(start_paused = true)]
async fn retryable_errors_are_retried_until_exhausted() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let counter = attempts.clone();
    let worker = from_fn(move |_: Context<Noop>| {
        counter.fetch_add(1, Ordering::SeqCst);
        async { Err::<(), _>(Error::new(ErrorKind::Http)) }
    });
    let crawler = Crawler::new(Noop, worker)
        .with_frontier(InMemFrontier::fifo().retry(2))
        .with_seed(seed("down"));

    let stats = crawler.run().await.unwrap();
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
    assert_eq!((stats.requests, stats.retried, stats.failed), (3, 2, 1));
}

#[tokio::test]
async fn retries_fail_without_a_retry_facade() {
    let worker = from_fn(|_: Context<Noop>| async { Outcome::retry() });
    let crawler = Crawler::new(Noop, worker)
        .with_frontier(InMemFrontier::fifo())
        .with_seed(seed("once"));

    let stats = crawler.run().await.unwrap();
    assert_eq!((stats.requests, stats.retried, stats.failed), (1, 0, 1));
}

#[tokio::test]
async fn non_retryable_errors_fail() {
    let worker = from_fn(|_: Context<Noop>| async { Err::<(), _>(Error::invalid("bad page")) });
    let crawler = Crawler::new(Noop, worker)
        .with_frontier(InMemFrontier::fifo().retry(3))
        .with_seed(seed("bad"));

    let stats = crawler.run().await.unwrap();
    assert_eq!((stats.requests, stats.failed, stats.retried), (1, 1, 0));
}

#[tokio::test(start_paused = true)]
async fn backoff_pauses_the_tag_and_retries() {
    let order = InMemDataset::<String>::new();
    let worker = from_fn(|cx: Context<Noop>| async move {
        let path = path(&cx);
        if path == "limited" && cx.request().attempt().get() == 0 {
            return Ok(Outcome::backoff(Duration::from_secs(10)));
        }
        cx.dataset::<String>()?.write(path).await?;
        Ok::<_, Error>(Outcome::Continue)
    });
    let crawler = Crawler::new(Noop, worker)
        .with_concurrency(1)
        .with_frontier(InMemFrontier::fifo().retry(1).tag_backoff())
        .with_dataset(order.clone())
        .with_seeds([
            seed("limited").with_tag("api"),
            seed("same-tag").with_tag("api"),
            seed("other").with_tag("web"),
        ]);

    let started = Instant::now();
    let stats = crawler.run().await.unwrap();
    assert_eq!(stats.retried, 1);
    assert_eq!(started.elapsed(), Duration::from_secs(10));

    let order = crawler
        .dataset::<String>()
        .unwrap()
        .read_all()
        .await
        .unwrap();
    assert_eq!(order, ["other", "same-tag", "limited"]);
}

#[tokio::test]
async fn stop_lets_in_flight_requests_finish() {
    let worker = from_fn(|cx: Context<Noop>| async move {
        if path(&cx) == "stop" {
            return Outcome::Stop;
        }
        cx.queue()
            .follow("https://example.com/never")
            .await
            .unwrap();
        Outcome::Continue
    });
    let crawler = Crawler::new(Noop, worker)
        .with_concurrency(1)
        .with_seeds([seed("stop"), seed("queued")]);

    let stats = crawler.run().await.unwrap();
    assert_eq!(stats.requests, 1);
    assert_eq!(crawler.queue().size_hint(), Some(1));
}

#[tokio::test(start_paused = true)]
async fn shutdown_token_stops_the_run() {
    let worker = from_fn(|cx: Context<Noop>| async move {
        sleep(Duration::from_secs(1)).await;
        cx.queue()
            .follow("https://example.com/next")
            .await
            .map(drop)
    });
    let crawler = Crawler::new(Noop, worker).with_seed(seed("start"));

    let token = crawler.shutdown_token();
    tokio::spawn(async move {
        sleep(Duration::from_millis(2500)).await;
        token.cancel();
    });

    let stats = crawler.run().await.unwrap();
    assert_eq!(stats.requests, 3);
}

#[tokio::test]
async fn panicking_worker_counts_as_failure() {
    let worker = from_fn(|cx: Context<Noop>| async move {
        assert_ne!(path(&cx), "panic", "worker bug");
    });
    let crawler = Crawler::new(Noop, worker).with_seeds([seed("panic"), seed("fine")]);

    let stats = crawler.run().await.unwrap();
    assert_eq!((stats.requests, stats.failed, stats.succeeded), (2, 1, 1));
}

#[tokio::test]
async fn unregistered_dataset_fails_the_request() {
    let worker = from_fn(|cx: Context<Noop>| async move { cx.dataset::<u64>()?.write(1).await });
    let crawler = Crawler::new(Noop, worker).with_seed(seed(""));

    let stats = crawler.run().await.unwrap();
    assert_eq!(stats.failed, 1);
}

#[tokio::test]
async fn run_can_be_spawned() {
    let crawler =
        Arc::new(Crawler::new(Noop, from_fn(|_: Context<Noop>| async {})).with_seed(seed("")));
    let handle = tokio::spawn(async move { crawler.run().await });
    assert_eq!(handle.await.unwrap().unwrap().requests, 1);
}

#[tokio::test(start_paused = true)]
async fn default_frontier_retries_transient_errors() {
    let worker = from_fn(|cx: Context<Noop>| async move {
        if cx.request().attempt().get() == 0 {
            return Err(Error::new(ErrorKind::Timeout));
        }
        Ok(())
    });
    let crawler = Crawler::new(Noop, worker).with_seed(seed("flaky"));

    let stats = crawler.run().await.unwrap();
    assert_eq!((stats.requests, stats.retried, stats.succeeded), (2, 1, 1));
}

#[tokio::test(start_paused = true)]
async fn retry_after_errors_back_off() {
    let worker = from_fn(|cx: Context<Noop>| async move {
        if cx.request().attempt().get() == 0 {
            let error = Error::new(ErrorKind::Http).with_retry_after(Duration::from_secs(30));
            return Err(error);
        }
        Ok(())
    });
    let crawler = Crawler::new(Noop, worker)
        .with_frontier(InMemFrontier::fifo().retry(1).tag_backoff())
        .with_seed(seed("limited"));

    let started = Instant::now();
    let stats = crawler.run().await.unwrap();
    assert_eq!((stats.requests, stats.retried), (2, 1));
    assert_eq!(started.elapsed(), Duration::from_secs(30));
}

#[tokio::test]
async fn rejected_errors_are_not_retried() {
    let worker =
        from_fn(|_: Context<Noop>| async { Err::<(), _>(Error::new(ErrorKind::Rejected)) });
    let crawler = Crawler::new(Noop, worker).with_seed(seed("gone"));

    let stats = crawler.run().await.unwrap();
    assert_eq!((stats.requests, stats.failed), (1, 1));
}

mod limits {
    use std::future::{Future, ready};
    use std::sync::Mutex;

    use super::*;
    use crate::frontier::{Ack, Admission, Frontier, Pop, Report};
    use crate::sync::lock;

    fn host_seed(host: &str, path: &str) -> Seed {
        Seed::get(format!("https://{host}/{path}")).unwrap()
    }

    /// A frontier recording every report, failing pops after `fail_after`.
    #[derive(Default)]
    struct Recording {
        inner: InMemFrontier,
        reports: Mutex<Vec<String>>,
        pops: AtomicUsize,
        fail_after: Option<usize>,
    }

    impl Frontier for Arc<Recording> {
        fn push(&self, seed: Seed) -> impl Future<Output = crate::Result<Admission>> + Send {
            self.inner.push(seed)
        }

        fn pop(&self) -> impl Future<Output = crate::Result<Pop>> + Send {
            let pops = self.pops.fetch_add(1, Ordering::SeqCst);
            if self.fail_after.is_some_and(|after| pops >= after) {
                return futures::future::Either::Left(ready(Err(Error::new(ErrorKind::Frontier))));
            }
            futures::future::Either::Right(self.inner.pop())
        }

        fn report(&self, report: Report) -> impl Future<Output = crate::Result<Ack>> + Send {
            let entry = match &report {
                Report::Done(seed) => format!("done {}", seed.uri().path()),
                Report::Failed(seed) => format!("failed {}", seed.uri().path()),
                Report::Skipped(seed) => format!("skipped {}", seed.uri().path()),
                Report::Cancelled(seed) => format!("cancelled {}", seed.uri().path()),
                Report::Retry { seed, .. } => format!("retry {}", seed.uri().path()),
                Report::Backoff { .. } => "backoff".to_owned(),
            };
            lock(&self.reports).push(entry);
            self.inner.report(report)
        }
    }

    #[tokio::test]
    async fn request_budget_stops_the_run() {
        let worker = from_fn(|cx: Context<Noop>| async move {
            cx.queue()
                .follow("https://example.com/next")
                .await
                .map(drop)
        });
        let crawler = Crawler::new(Noop, worker)
            .with_max_requests(3)
            .with_seed(seed("start"));

        let stats = crawler.run().await.unwrap();
        assert_eq!(stats.requests, 3);
    }

    #[tokio::test(start_paused = true)]
    async fn time_limit_stops_the_run() {
        let worker = from_fn(|cx: Context<Noop>| async move {
            sleep(Duration::from_secs(1)).await;
            cx.queue()
                .follow("https://example.com/next")
                .await
                .map(drop)
        });
        let crawler = Crawler::new(Noop, worker)
            .with_time_limit(Duration::from_millis(3500))
            .with_seed(seed("start"));

        let stats = crawler.run().await.unwrap();
        assert_eq!(stats.requests, 4);
    }

    #[tokio::test(start_paused = true)]
    async fn request_timeout_fails_and_retries() {
        let worker = from_fn(|cx: Context<Noop>| async move {
            if cx.request().attempt().get() == 0 {
                sleep(Duration::from_mins(1)).await;
            }
        });
        let crawler = Crawler::new(Noop, worker)
            .with_request_timeout(Duration::from_secs(5))
            .with_seed(seed("slow"));

        let stats = crawler.run().await.unwrap();
        assert_eq!((stats.requests, stats.retried, stats.succeeded), (2, 1, 1));
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_grace_cancels_in_flight_requests() {
        let worker = from_fn(|_: Context<Noop>| async {
            sleep(Duration::from_mins(10)).await;
        });
        let frontier = Arc::new(Recording::default());
        let crawler = Crawler::new(Noop, worker)
            .with_frontier(frontier.clone())
            .with_shutdown_grace(Duration::from_secs(1))
            .with_seeds([seed("a"), seed("b")]);

        let token = crawler.shutdown_token();
        tokio::spawn(async move {
            sleep(Duration::from_secs(1)).await;
            token.cancel();
        });

        let started = Instant::now();
        let stats = crawler.run().await.unwrap();
        assert_eq!(started.elapsed(), Duration::from_secs(2));
        assert_eq!((stats.cancelled, stats.requests), (2, 0));

        let mut reports = lock(&frontier.reports).clone();
        reports.sort();
        assert_eq!(reports, ["cancelled /a", "cancelled /b"]);
    }

    #[tokio::test(start_paused = true)]
    async fn host_concurrency_limits_each_host() {
        let worker = from_fn(|_: Context<Noop>| async {
            sleep(Duration::from_millis(100)).await;
        });
        let seeds = (0..4).flat_map(|i| {
            [
                host_seed("a.com", &i.to_string()),
                host_seed("b.com", &i.to_string()),
            ]
        });
        let crawler = Crawler::new(Noop, worker)
            .with_concurrency(8)
            .with_host_concurrency(1)
            .with_seeds(seeds);

        let started = Instant::now();
        let stats = crawler.run().await.unwrap();
        assert_eq!(stats.requests, 8);
        assert_eq!(started.elapsed(), Duration::from_millis(400));
    }

    #[tokio::test(start_paused = true)]
    async fn host_delay_spaces_out_requests() {
        let worker = from_fn(|_: Context<Noop>| async {});
        let crawler = Crawler::new(Noop, worker)
            .with_host_delay(Duration::from_secs(1))
            .with_seeds([
                host_seed("a.com", "1"),
                host_seed("a.com", "2"),
                host_seed("a.com", "3"),
                host_seed("b.com", "1"),
            ]);

        let started = Instant::now();
        let stats = crawler.run().await.unwrap();
        assert_eq!(stats.requests, 4);
        assert_eq!(started.elapsed(), Duration::from_secs(2));
    }

    #[tokio::test]
    async fn abort_skips_the_rest_of_the_tag() {
        let worker = from_fn(|cx: Context<Noop>| async move {
            if cx.request().uri().path() == "/first" {
                Outcome::abort()
            } else {
                Outcome::Continue
            }
        });
        let crawler = Crawler::new(Noop, worker).with_concurrency(1).with_seeds([
            seed("first").with_tag("doomed"),
            seed("second").with_tag("doomed"),
            seed("other").with_tag("kept"),
            seed("third").with_tag("doomed"),
        ]);

        let stats = crawler.run().await.unwrap();
        assert_eq!((stats.requests, stats.skipped), (2, 2));
    }

    #[tokio::test]
    async fn frontier_errors_drain_and_fail_the_run() {
        let finished = Arc::new(AtomicUsize::new(0));
        let counter = finished.clone();
        let worker = from_fn(move |_: Context<Noop>| {
            let counter = counter.clone();
            async move {
                tokio::task::yield_now().await;
                counter.fetch_add(1, Ordering::SeqCst);
            }
        });
        let frontier = Arc::new(Recording {
            fail_after: Some(2),
            ..Recording::default()
        });
        let crawler = Crawler::new(Noop, worker)
            .with_frontier(frontier.clone())
            .with_seeds([seed("a"), seed("b"), seed("c")]);

        let error = crawler.run().await.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Frontier);
        assert_eq!(finished.load(Ordering::SeqCst), 2);
        assert_eq!(crawler.stats().requests, 2);
        assert_eq!(lock(&frontier.reports).len(), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn stats_are_live() {
        let worker = from_fn(|cx: Context<Noop>| async move {
            sleep(Duration::from_secs(1)).await;
            if cx.request().depth().get() < 3 {
                cx.queue().follow("https://example.com/next").await?;
            }
            Ok::<_, Error>(())
        });
        let crawler = Arc::new(Crawler::new(Noop, worker).with_seed(seed("start")));
        let running = tokio::spawn({
            let crawler = crawler.clone();
            async move { crawler.run().await }
        });

        sleep(Duration::from_millis(2500)).await;
        let stats = crawler.stats();
        assert_eq!(stats.requests, 2);
        assert_eq!(stats.elapsed, Duration::from_millis(2500));

        let finished = running.await.unwrap().unwrap();
        assert_eq!(finished.requests, 4);
        assert_eq!(crawler.stats(), finished);
    }
}

#[cfg(feature = "robots")]
mod robots {
    use std::collections::{HashMap, VecDeque};
    use std::future::{Future, ready};
    use std::sync::Mutex;

    use http::StatusCode;

    use super::*;
    use crate::backend::{Backend, Client};
    use crate::crawler::Robots;
    use crate::request::{Request, Response};
    use crate::sync::lock;

    /// The robots.txt responses of each host: statuses and bodies.
    type Scripts = HashMap<String, VecDeque<(u16, &'static str)>>;

    /// Serves robots.txt per host from a script of statuses and bodies, the
    /// last one repeating, and counts the fetches.
    #[derive(Clone, Default)]
    struct Site {
        robots: Arc<Mutex<Scripts>>,
        fetches: Arc<AtomicUsize>,
    }

    impl Site {
        fn serve(self, host: &str, script: &[(u16, &'static str)]) -> Self {
            lock(&self.robots).insert(host.to_owned(), script.iter().copied().collect());
            self
        }

        fn fetches(&self) -> usize {
            self.fetches.load(Ordering::SeqCst)
        }
    }

    impl Backend for Site {
        type Client = Page;

        fn connect(&self) -> impl Future<Output = crate::Result<Page>> + Send {
            let page = Page {
                site: self.clone(),
                body: Mutex::new(""),
            };
            ready(Ok(page))
        }
    }

    /// A request to a [`Site`], keeping the body it was served.
    struct Page {
        site: Site,
        body: Mutex<&'static str>,
    }

    impl Client for Page {
        fn fetch(
            &self,
            request: &Request,
        ) -> impl Future<Output = crate::Result<Response<()>>> + Send {
            if request.uri().path() != "/robots.txt" {
                *lock(&self.body) = "page";
                return ready(Ok(Response::new(())));
            }

            self.site.fetches.fetch_add(1, Ordering::SeqCst);
            let host = request.uri().host().unwrap_or_default().to_owned();
            let mut robots = lock(&self.site.robots);
            let script = robots.entry(host).or_default();
            let (status, body) = match script.len() {
                0 => (404, ""),
                1 => script[0],
                _ => script.pop_front().unwrap_or((404, "")),
            };

            *lock(&self.body) = body;
            let mut response = Response::new(());
            *response.status_mut() = StatusCode::from_u16(status).unwrap();
            ready(Ok(response))
        }

        fn body(&self) -> impl Future<Output = crate::Result<bytes::Bytes>> + Send {
            ready(Ok(bytes::Bytes::from_static(lock(&self.body).as_bytes())))
        }
    }

    fn page(uri: &str) -> Seed {
        Seed::get(uri).unwrap()
    }

    const PRIVATE: &str = "User-agent: *\nDisallow: /private\n";

    #[tokio::test(start_paused = true)]
    async fn obeys_robots_txt() {
        let site = Site::default().serve(
            "example.com",
            &[(
                200,
                "User-agent: spire\nDisallow: /private\nCrawl-delay: 1\n",
            )],
        );
        let crawler = Crawler::new(site, from_fn(|_: Context<Page>| async {}))
            .with_robots("spire")
            .with_seeds([seed("a"), seed("private/b"), seed("c")]);

        let started = Instant::now();
        let stats = crawler.run().await.unwrap();
        assert_eq!((stats.requests, stats.skipped), (2, 1));
        // `Crawl-delay: 1` spaces out the two allowed requests.
        assert_eq!(started.elapsed(), Duration::from_secs(1));
    }

    #[tokio::test]
    async fn fetches_with_a_separate_backend() {
        let crawl = Site::default().serve("example.com", &[(200, "")]);
        let robots = Site::default().serve("example.com", &[(200, PRIVATE)]);
        let crawler = Crawler::new(crawl.clone(), from_fn(|_: Context<Page>| async {}))
            .with_robots(Robots::new("bot").with_backend(robots.clone()))
            .with_seeds([seed("public"), seed("private/page")]);

        let stats = crawler.run().await.unwrap();
        assert_eq!((stats.requests, stats.skipped), (1, 1));
        assert_eq!((crawl.fetches(), robots.fetches()), (0, 1));
    }

    #[tokio::test(start_paused = true)]
    async fn waits_while_unreachable_then_obeys() {
        let site = Site::default().serve("example.com", &[(503, ""), (503, ""), (200, PRIVATE)]);
        let crawler = Crawler::new(site.clone(), from_fn(|_: Context<Page>| async {}))
            .with_robots("bot")
            .with_seeds([seed("public"), seed("private/page")]);

        let started = Instant::now();
        let stats = crawler.run().await.unwrap();
        assert_eq!((stats.requests, stats.skipped), (1, 1));
        assert_eq!(site.fetches(), 3);
        assert_eq!(started.elapsed(), Duration::from_secs(30 + 60));
    }

    #[tokio::test(start_paused = true)]
    async fn skips_an_origin_that_stays_unreachable() {
        let site = Site::default()
            .serve("down.com", &[(500, "")])
            .serve("up.com", &[(200, "")]);
        let crawler = Crawler::new(site.clone(), from_fn(|_: Context<Page>| async {}))
            .with_robots(Robots::new("bot").with_unreachable_retries(2))
            .with_seeds([page("https://down.com/a"), page("https://up.com/b")]);

        let started = Instant::now();
        let stats = crawler.run().await.unwrap();
        assert_eq!((stats.requests, stats.skipped), (1, 1));
        assert_eq!(site.fetches(), 1 + 3);
        assert_eq!(started.elapsed(), Duration::from_secs(30 + 60));
    }

    #[tokio::test]
    async fn missing_robots_txt_allows_everything() {
        let crawler = Crawler::new(Site::default(), from_fn(|_: Context<Page>| async {}))
            .with_robots("bot")
            .with_seeds([seed("a"), seed("private/b")]);

        let stats = crawler.run().await.unwrap();
        assert_eq!((stats.requests, stats.skipped), (2, 0));
    }

    #[tokio::test(start_paused = true)]
    async fn too_many_requests_counts_as_unreachable() {
        let site = Site::default().serve("example.com", &[(429, ""), (200, "")]);
        let crawler = Crawler::new(site, from_fn(|_: Context<Page>| async {}))
            .with_robots("bot")
            .with_seed(seed("a"));

        let started = Instant::now();
        assert_eq!(crawler.run().await.unwrap().requests, 1);
        assert_eq!(started.elapsed(), Duration::from_secs(30));
    }

    #[tokio::test(start_paused = true)]
    async fn refetches_expired_rules() {
        let worker = from_fn(|cx: Context<Page>| async move {
            sleep(Duration::from_mins(40)).await;
            if cx.request().depth().get() < 2 {
                cx.queue().follow("next").await?;
            }
            Ok::<_, Error>(())
        });
        let site = Site::default().serve("example.com", &[(200, "")]);
        let crawler = Crawler::new(site.clone(), worker)
            .with_robots(Robots::new("bot").with_max_age(Duration::from_hours(1)))
            .with_seed(seed("start"));

        let stats = crawler.run().await.unwrap();
        assert_eq!(stats.requests, 3);
        // Fetched at the start, and again for the request at 80 minutes.
        assert_eq!(site.fetches(), 2);
    }

    #[tokio::test]
    async fn schemes_have_separate_robots_txt() {
        let site = Site::default();
        let crawler = Crawler::new(site.clone(), from_fn(|_: Context<Page>| async {}))
            .with_robots("bot")
            .with_seeds([page("http://example.com/a"), page("https://example.com/b")]);

        assert_eq!(crawler.run().await.unwrap().requests, 2);
        assert_eq!(site.fetches(), 2);
    }
}

mod resume {
    use super::*;
    use crate::dataset::Data;
    use crate::frontier::seed::Fingerprint;
    use crate::frontier::seen::InMemSeen;

    /// Visits `/0` through `/9`, each page linking to the next.
    async fn chain(cx: Context<Noop>) -> crate::Result<()> {
        let path = cx.request().uri().path().trim_start_matches('/').to_owned();
        let page: u32 = path.parse().unwrap();
        cx.dataset::<u32>()?.write(page).await?;
        if page < 9 {
            cx.queue()
                .follow(format!("https://example.com/{}", page + 1))
                .await?;
        }
        Ok(())
    }

    #[cfg(feature = "serde")]
    fn round_trip(seeds: &[Seed], seen: &[Fingerprint]) -> (Vec<Seed>, Vec<Fingerprint>) {
        let seeds = serde_json::to_string(seeds).unwrap();
        let seen = serde_json::to_string(seen).unwrap();
        (
            serde_json::from_str(&seeds).unwrap(),
            serde_json::from_str(&seen).unwrap(),
        )
    }

    #[cfg(not(feature = "serde"))]
    fn round_trip(seeds: &[Seed], seen: &[Fingerprint]) -> (Vec<Seed>, Vec<Fingerprint>) {
        (seeds.to_vec(), seen.to_vec())
    }

    async fn visited(data: Data<u32>) -> Vec<u32> {
        data.read_all().await.unwrap()
    }

    #[tokio::test]
    async fn snapshot_and_restore_resume_the_crawl() {
        let seen = Arc::new(InMemSeen::default());
        let first = Crawler::new(Noop, from_fn(chain))
            .with_concurrency(1)
            .with_max_requests(4)
            .with_frontier(InMemFrontier::fifo().dedup_with(seen.clone()))
            .with_dataset(InMemDataset::<u32>::new())
            .with_seed(seed("0"));
        first.run().await.unwrap();
        assert_eq!(visited(first.dataset().unwrap()).await, [0, 1, 2, 3]);

        let (seeds, fingerprints) = round_trip(&first.snapshot().unwrap(), &seen.snapshot());
        assert_eq!(seeds.len(), 1);
        assert_eq!(seeds[0].uri().path(), "/4");

        let seen: Arc<InMemSeen> = Arc::new(fingerprints.into_iter().collect());
        let frontier: InMemFrontier = seeds.into_iter().collect();
        let second = Crawler::new(Noop, from_fn(chain))
            .with_frontier(frontier.dedup_with(seen))
            .with_dataset(InMemDataset::<u32>::new())
            // Already seen: the restored seen set refuses it.
            .with_seed(seed("2"));
        let stats = second.run().await.unwrap();
        assert_eq!(stats.requests, 6);
        assert_eq!(visited(second.dataset().unwrap()).await, [4, 5, 6, 7, 8, 9]);
    }

    #[tokio::test(start_paused = true)]
    async fn snapshot_keeps_cancelled_and_retrying_seeds() {
        let worker = from_fn(|cx: Context<Noop>| async move {
            if cx.request().uri().path() == "/flaky" {
                return Outcome::retry_after(Duration::from_mins(1));
            }
            sleep(Duration::from_mins(10)).await;
            Outcome::Continue
        });
        let crawler = Crawler::new(Noop, worker)
            .with_shutdown_grace(Duration::from_secs(1))
            .with_seeds([seed("flaky"), seed("slow")]);

        let token = crawler.shutdown_token();
        tokio::spawn(async move {
            sleep(Duration::from_secs(1)).await;
            token.cancel();
        });
        crawler.run().await.unwrap();

        let mut pending: Vec<_> = crawler
            .snapshot()
            .unwrap()
            .into_iter()
            .map(|seed| (seed.uri().path().to_owned(), seed.attempt().get()))
            .collect();
        pending.sort();
        assert_eq!(pending, [("/flaky".to_owned(), 1), ("/slow".to_owned(), 0)]);
    }
}

/// Crawls twelve pages of one host, each answering `page` after 100ms,
/// with `concurrency`, and returns the most fetches in flight at once.
async fn peak_fetches(
    page: crate::testing::MockPage,
    concurrency: crate::crawler::HostConcurrency,
) -> usize {
    use std::sync::atomic::AtomicUsize;

    use crate::testing::{MockClient, MockSite};

    let page = page.with_latency(Duration::from_millis(100));
    let site = (0..12).fold(MockSite::new(), |site, i| {
        site.page(&format!("https://example.com/{i}"), page.clone())
    });
    let counts = Arc::new((AtomicUsize::new(0), AtomicUsize::new(0)));
    let worker = {
        let counts = Arc::clone(&counts);
        from_fn(move |cx: Context<MockClient>| {
            let counts = Arc::clone(&counts);
            async move {
                let (now, peak) = &*counts;
                let in_flight = now.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(in_flight, Ordering::SeqCst);
                let response = cx.response().await.map(drop);
                now.fetch_sub(1, Ordering::SeqCst);
                response
            }
        })
    };
    let crawler = Crawler::new(site, worker)
        .with_frontier(InMemFrontier::fifo())
        .with_host_concurrency(concurrency)
        .with_seeds((0..12).map(|i| seed(&i.to_string())));
    crawler.run().await.unwrap();
    counts.1.load(Ordering::SeqCst)
}

#[tokio::test(start_paused = true)]
async fn host_concurrency_is_fixed_or_adapts() {
    use http::StatusCode;

    use crate::crawler::HostConcurrency;
    use crate::testing::MockPage;

    // A fixed limit is reached at once.
    let fixed = peak_fetches(MockPage::text(""), 3.into()).await;
    assert_eq!(fixed, 3);

    // An adaptive one grows from one to its maximum with every success.
    let healthy = peak_fetches(MockPage::text(""), HostConcurrency::adaptive(4)).await;
    assert_eq!(healthy, 4);

    // A host asking to slow down keeps it at one.
    let busy = MockPage::status(StatusCode::SERVICE_UNAVAILABLE);
    let busy = peak_fetches(busy, HostConcurrency::adaptive(4)).await;
    assert_eq!(busy, 1);
}

/// Crawls `seeds` on `site` one at a time and without retries, returning
/// the time each fetch started, by path, after the crawl's start.
async fn fetch_times(
    site: crate::testing::MockSite,
    seeds: impl IntoIterator<Item = Seed>,
) -> Vec<(String, Duration)> {
    use std::sync::Mutex;

    use crate::testing::MockClient;

    let start = Instant::now();
    let times = Arc::new(Mutex::new(Vec::new()));
    let worker = {
        let times = Arc::clone(&times);
        from_fn(move |cx: Context<MockClient>| {
            let path = cx.request().uri().path().to_owned();
            times.lock().unwrap().push((path, start.elapsed()));
            async move { cx.response().await.map(drop) }
        })
    };
    Crawler::new(site, worker)
        .with_frontier(InMemFrontier::fifo())
        .with_host_concurrency(1)
        .with_seeds(seeds)
        .run()
        .await
        .unwrap();
    Arc::try_unwrap(times).unwrap().into_inner().unwrap()
}

#[tokio::test(start_paused = true)]
async fn throttled_hosts_get_wider_gaps() {
    use http::StatusCode;

    use crate::testing::{MockPage, MockSite};

    let busy =
        MockPage::status(StatusCode::SERVICE_UNAVAILABLE).with_latency(Duration::from_millis(100));
    let site = (0..5).fold(MockSite::new(), |site, i| {
        site.page(&format!("https://example.com/{i}"), busy.clone())
    });
    let times = fetch_times(site, (0..5).map(|i| seed(&i.to_string()))).await;

    // The first failure widens the gap to 1 s, and each after it doubles
    // it, counted from the last start.
    let starts: Vec<_> = times.iter().map(|(_, at)| at.as_millis()).collect();
    assert_eq!(starts, [0, 1000, 3000, 7000, 15000]);
}

#[tokio::test(start_paused = true)]
async fn retry_after_pauses_the_whole_host() {
    use http::header::RETRY_AFTER;
    use http::{HeaderValue, StatusCode};

    use crate::testing::{MockPage, MockSite};

    let limited = MockPage::status(StatusCode::TOO_MANY_REQUESTS)
        .with_header(RETRY_AFTER, HeaderValue::from_static("5"));
    let site = MockSite::new()
        .page("https://example.com/limited", limited)
        .page("https://example.com/other", MockPage::text(""));
    let seeds = [
        seed("limited").with_tag("search"),
        seed("other").with_tag("page"),
    ];
    let times = fetch_times(site, seeds).await;

    // The host asked for 5 s, so the other tag's request waits too.
    let other = times.iter().find(|(path, _)| path == "/other").unwrap();
    assert_eq!(other.1, Duration::from_secs(5));
}

#[tokio::test(start_paused = true)]
async fn retry_after_pauses_without_widening_the_gap() {
    use http::header::RETRY_AFTER;
    use http::{HeaderValue, StatusCode};

    use crate::testing::{MockPage, MockSite};

    let limited = MockPage::status(StatusCode::TOO_MANY_REQUESTS)
        .with_header(RETRY_AFTER, HeaderValue::from_static("1"));
    let site = MockSite::new()
        .page("https://example.com/limited", limited)
        .page("https://example.com/a", MockPage::text(""))
        .page("https://example.com/b", MockPage::text(""));
    let times = fetch_times(site, [seed("limited"), seed("a"), seed("b")]).await;

    // Paused for the second the host asked for, then back at full pace: a
    // widened gap would hold `/b` for another second.
    let starts: Vec<_> = times
        .iter()
        .map(|(path, at)| (path.as_str(), at.as_millis()))
        .collect();
    assert_eq!(starts, [("/limited", 0), ("/a", 1000), ("/b", 1000)]);
}

#[test]
fn only_the_host_throttles_it() {
    use super::run::signal;
    use crate::crawler::concurrency::Signal;

    let unavailable =
        Error::new(ErrorKind::Http).with_status(http::StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(signal(&Outcome::Fail(unavailable)), Signal::Throttled);
    assert_eq!(
        signal(&Outcome::Fail(Error::new(ErrorKind::Timeout))),
        Signal::Throttled
    );
    // A handler's backoff pauses its tag, not the host.
    assert_eq!(
        signal(&Outcome::backoff(Duration::from_secs(5))),
        Signal::Failed
    );
    assert_eq!(signal(&Outcome::Continue), Signal::Ok);
}

#[tokio::test(start_paused = true)]
async fn revisits_keep_the_crawl_going_until_it_is_stopped() {
    use std::sync::Mutex;

    use crate::request::{Revisit, Tag, Tags};

    #[derive(Clone, Copy)]
    struct Listing;

    impl From<Listing> for Tag {
        fn from(_: Listing) -> Self {
            Self::from_static("listing")
        }
    }

    impl Tags for Listing {
        const ALL: &'static [Self] = &[Self];
    }

    impl Revisit for Listing {
        fn interval(self) -> Option<Duration> {
            Some(Duration::from_mins(1))
        }
    }

    let start = Instant::now();
    let visits = Arc::new(Mutex::new(Vec::new()));
    let worker = {
        let visits = Arc::clone(&visits);
        from_fn(move |_: Context<Noop>| {
            visits.lock().unwrap().push(start.elapsed().as_secs());
            async {}
        })
    };
    let crawler = Crawler::new(Noop, worker)
        .with_frontier(InMemFrontier::fifo().revisit::<Listing>().dedup())
        .with_time_limit(Duration::from_secs(150))
        .with_seed(seed("list").with_tag(Listing));
    let stats = crawler.run().await.unwrap();

    // Crawled at once, then every minute, until the time limit.
    assert_eq!(*visits.lock().unwrap(), [0, 60, 120]);
    assert_eq!(stats.succeeded, 3);
}
