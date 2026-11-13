//! The run loop of a [`Crawler`].

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::pin::pin;
use std::sync::Arc;
use std::time::Duration;

use ::http::StatusCode;
use tokio::sync::mpsc;
use tokio::task::{Id, JoinError, JoinSet};
use tokio::time::{Instant, sleep_until, timeout};
use tracing::Instrument;

use crate::backend::Backend;
use crate::backend::redirect::{RedirectRequest, Redirects, TransferEvent, cancelled};
use crate::context::{Context, FETCH, Fetch, FetchScope};
use crate::crawler::admission::Pending;
use crate::crawler::concurrency::{Feedback, Signal};
use crate::crawler::connector::Connector;
use crate::crawler::discovery::Discovery;
use crate::crawler::event::{
    Emitter, Event, Failure, OutcomeKind, RequestInfo, SkipReason, StopReason,
};
use crate::crawler::hosts::{Admit, Hosts};
#[cfg(feature = "robots")]
use crate::crawler::robots::{BackendSource, RobotsCache, RobotsSource, Verdict};
use crate::crawler::{Crawler, Live, Stats, Tag};
use crate::frontier::queue::Queue;
use crate::frontier::seed::Seed;
use crate::frontier::{Ack, NextSeed, Report};
use crate::identity::{Health, Identity};
use crate::sync::lock;
use crate::worker::{Outcome, Scope, Worker};
use crate::{Error, ErrorKind, Result, TRACING_TARGET};

/// Seeds parked for busy hosts at most, before the frontier stops being
/// read; bounds memory when one host dominates the frontier.
const MAX_PARKED: usize = 4096;

/// The robots.txt state of a run.
#[cfg(feature = "robots")]
struct RobotsRun {
    cache: RobotsCache,
    source: Arc<dyn RobotsSource>,
    agent: Arc<str>,
    tasks: JoinSet<(String, Result<robotxt::Robots>)>,
    fetches: HashMap<String, tokio::task::AbortHandle>,
}

/// A fetched robots.txt: the origin and its rules, or why it is unreachable.
#[cfg(feature = "robots")]
enum RobotsEvent {
    Fetched {
        task: Id,
        origin: String,
        result: Result<robotxt::Robots>,
    },
    Cancelled,
}

/// Without the `robots` feature, no robots.txt state exists.
#[cfg(not(feature = "robots"))]
enum RobotsRun {}

#[cfg(not(feature = "robots"))]
type RobotsEvent = std::convert::Infallible;

/// Waits for the next fetched robots.txt; never resolves without fetches.
async fn next_robots(robots: &mut Option<RobotsRun>) -> RobotsEvent {
    #[cfg(feature = "robots")]
    if let Some(robots) = robots
        && let Some(joined) = robots.tasks.join_next_with_id().await
    {
        return match joined {
            Ok((task, (origin, result))) => RobotsEvent::Fetched {
                task,
                origin,
                result,
            },
            Err(error) => match robots
                .fetches
                .iter()
                .find(|(_, handle)| handle.id() == error.id())
            {
                Some((origin, _)) => RobotsEvent::Fetched {
                    task: error.id(),
                    origin: origin.clone(),
                    result: Err(Error::backend("robots fetch task failed").with_source(error)),
                },
                None => RobotsEvent::Cancelled,
            },
        };
    }
    #[cfg(not(feature = "robots"))]
    let _ = robots;
    std::future::pending().await
}

/// Whether robots.txt lets a seed be crawled.
#[cfg_attr(
    not(feature = "robots"),
    expect(dead_code, reason = "robots.txt only disallows with `robots`")
)]
enum Access {
    Allow(Pending),
    Disallow(Pending),
    Parked,
}

/// The state of a single run.
struct Run<B> {
    tasks: JoinSet<(Outcome, Fetch)>,
    redirects: mpsc::UnboundedReceiver<TransferEvent>,
    redirect_sender: mpsc::UnboundedSender<TransferEvent>,
    /// The running requests.
    in_flight: HashMap<Id, InFlight>,
    robots: Option<RobotsRun>,
    hosts: Hosts,
    /// Connects for requests and robots.txt, as an identity or plainly.
    connector: Arc<Connector<B>>,
    events: Emitter,
    aborted: HashSet<Option<Tag>>,
    discovery: Discovery,
    started: usize,
    stopping: bool,
    grace: Option<Instant>,
    error: Option<Error>,
}

/// Ownership of a host slot during one logical request.
enum HostSlot {
    Untracked,
    Held(String),
    WaitingForRedirect,
}

impl HostSlot {
    fn active(key: Option<String>) -> Self {
        key.map_or(Self::Untracked, Self::Held)
    }
}

/// A running request.
struct InFlight {
    seed: Seed,
    destination: Option<Box<Seed>>,
    /// The identity it went out as, if any.
    identity: Option<Identity>,
    /// A parked redirect owns no host slot and receives no fetch feedback.
    host: HostSlot,
    /// Last admitted transfer and its measured network time.
    transfer: Option<(Instant, Option<Duration>)>,
    /// When the logical request started.
    started: Instant,
}

/// The event for a request handled with `outcome`.
fn handled(request: RequestInfo, outcome: &Outcome, elapsed: Duration) -> Event {
    let error = match outcome {
        Outcome::Fail(error) | Outcome::Fatal(error) => Some(Failure::from(error)),
        _ => None,
    };
    Event::Handled {
        request,
        outcome: OutcomeKind::from(outcome),
        error,
        elapsed,
    }
}

/// How a request went, as far as its host is concerned: a host answering
/// `429` or `503`, or timing out, is throttling the crawl. A handler's
/// backoff pauses its tag, not the host, so it is neither a success nor
/// throttling.
pub(super) fn signal(outcome: &Outcome) -> Signal {
    match outcome {
        Outcome::Fail(error)
            if error.kind() == ErrorKind::Timeout
                || matches!(
                    error.status().map(|status| status.as_u16()),
                    Some(429 | 503)
                ) =>
        {
            Signal::Throttled
        }
        Outcome::Fail(_) | Outcome::Fatal(_) | Outcome::Retry { .. } | Outcome::Backoff { .. } => {
            Signal::Failed
        }
        Outcome::Continue | Outcome::Abort(_) | Outcome::Stop => Signal::Ok,
    }
}

impl<B: Backend> Run<B> {
    fn prune_admissions(&mut self) {
        let in_flight = &self.in_flight;
        let active = |pending: &Pending| {
            !pending.is_cancelled()
                && pending
                    .task()
                    .is_none_or(|task| in_flight.contains_key(&task))
        };
        self.hosts.retain(active);
        #[cfg(feature = "robots")]
        if let Some(robots) = &mut self.robots {
            for origin in robots.cache.retain(active) {
                if let Some(fetch) = robots.fetches.remove(&origin) {
                    fetch.abort();
                }
            }
        }
    }

    fn resume_redirect(&mut self, redirect: RedirectRequest) {
        let RedirectRequest { task, seed, answer } = redirect;
        let Some(in_flight) = self.in_flight.get_mut(&task) else {
            let _ = answer.send(Err(cancelled()));
            return;
        };
        // No await between delivering the permit and reserving its slot:
        // the scheduler cannot admit competing work in between. A cancelled
        // receiver therefore never consumes capacity or a host delay.
        if answer.send(Ok(())).is_err() {
            return;
        }
        in_flight.host = HostSlot::active(self.hosts.start(&seed, Instant::now()));
        in_flight.destination = Some(Box::new(seed));
        in_flight.transfer = Some((Instant::now(), None));
    }

    /// Records that a request for `seed` to `host` finished with `outcome`:
    /// a throttled host is slowed down, and one with `Retry-After` paused.
    fn finish_host(
        &mut self,
        host: Option<String>,
        seed: &Seed,
        started: Instant,
        fetch: Fetch,
        outcome: &Outcome,
    ) {
        let now = Instant::now();
        // A retryable error with `Retry-After` pauses the host it came from
        // for exactly as long, without slowing it down further: the host
        // said how long to wait, so there is nothing to guess.
        let pause = match outcome {
            Outcome::Fail(error) if error.is_retryable() => error.retry_after(),
            _ => None,
        };
        let signal = if pause.is_some() {
            Signal::Failed
        } else {
            signal(outcome)
        };
        // Hosts are tracked as soon as one is throttled, so it slows down
        // even in a crawl without limits or gaps.
        let throttled = signal == Signal::Throttled || pause.is_some();
        let Some(host) = host.or_else(|| throttled.then(|| self.hosts.track(seed, now))) else {
            return;
        };
        let feedback = Feedback {
            started,
            latency: fetch.latency,
            unchanged: fetch.status == Some(StatusCode::NOT_MODIFIED),
            signal,
        };
        if let Some(slowdown) = self.hosts.finished(&host, Some(feedback), now) {
            self.events.emit(|| Event::HostSlowed {
                host: host.clone(),
                slowdown,
            });
        }
        if let Some(pause) = pause
            && self.hosts.pause(&host, now + pause)
        {
            self.events.emit(|| Event::HostPaused {
                host,
                duration: pause,
            });
        }
    }

    fn stop(&mut self, reason: StopReason, grace: Option<Duration>) {
        if self.stopping {
            return;
        }
        tracing::info!(target: TRACING_TARGET, in_flight = self.tasks.len(), %reason, "stopping crawl");
        self.events.emit(|| Event::Stopping { reason });
        self.stopping = true;
        self.grace = grace.map(|grace| Instant::now() + grace);
    }

    /// Stops the run after a fatal failure; the error is returned once
    /// in-flight requests are drained.
    fn fail(&mut self, error: Error, grace: Option<Duration>) {
        tracing::error!(target: TRACING_TARGET, %error, "crawl failed");
        self.error.get_or_insert(error);
        self.stop(StopReason::Failed, grace);
    }

    fn is_aborted(&self, seed: &Seed) -> bool {
        self.aborted.contains(&seed.tag().cloned())
    }

    /// Whether robots.txt fetches are in flight.
    #[cfg_attr(
        not(feature = "robots"),
        expect(clippy::unused_self, reason = "no robots.txt state")
    )]
    fn robots_busy(&self) -> bool {
        #[cfg(feature = "robots")]
        return self
            .robots
            .as_ref()
            .is_some_and(|robots| !robots.tasks.is_empty());
        #[cfg(not(feature = "robots"))]
        false
    }

    /// Asks robots.txt whether `seed` may be crawled.
    #[cfg_attr(
        not(feature = "robots"),
        expect(clippy::unused_self, reason = "no robots.txt state")
    )]
    fn robots_access(&mut self, seed: Pending) -> Access {
        #[cfg(feature = "robots")]
        if let Some(robots) = &mut self.robots {
            return match robots.cache.admit(seed, Instant::now()) {
                Verdict::Allow(seed) => Access::Allow(seed),
                Verdict::Disallow(seed) => Access::Disallow(seed),
                Verdict::Parked => Access::Parked,
            };
        }
        Access::Allow(seed)
    }

    /// Starts the robots.txt fetches due, returning when the next blocked
    /// origin is retried.
    #[cfg_attr(
        not(feature = "robots"),
        expect(clippy::unused_self, reason = "no robots.txt state")
    )]
    fn robots_tick(&mut self) -> Option<Instant> {
        #[cfg(feature = "robots")]
        if let Some(robots) = &mut self.robots {
            robots.cache.retry_due(Instant::now());
            for (origin, url) in robots.cache.take_fetches() {
                let fetch = robots.source.fetch(url, robots.agent.clone());
                let key = origin.clone();
                let handle = robots.tasks.spawn(async move { (origin, fetch.await) });
                robots.fetches.insert(key, handle);
            }
            return robots.cache.earliest();
        }
        None
    }

    /// The number of seeds parked for hosts or robots.txt.
    fn parked(&self) -> usize {
        #[cfg(feature = "robots")]
        let robots = self
            .robots
            .as_ref()
            .map_or(0, |robots| robots.cache.parked());
        #[cfg(not(feature = "robots"))]
        let robots = 0;
        self.hosts.parked() + robots
    }

    /// Takes every parked seed, e.g. when the run stops.
    fn drain(&mut self) -> Vec<Pending> {
        let mut seeds = self.hosts.drain();
        seeds.extend(self.discovery.drain().into_iter().map(Pending::Seed));
        #[cfg(feature = "robots")]
        let seeds = match &mut self.robots {
            Some(robots) => {
                robots.tasks.abort_all();
                seeds.into_iter().chain(robots.cache.drain()).collect()
            }
            None => seeds,
        };
        seeds
    }
}

impl<B, W> Crawler<B, W>
where
    B: Backend,
    W: Worker<B::Session>,
{
    /// Crawls until the frontier is empty and no request is in flight, a
    /// worker returns [`Outcome::Stop`], the budget is spent, or the run is
    /// shut down.
    ///
    /// # Errors
    ///
    /// Returns an error if a seed cannot be pushed, or if the frontier fails
    /// mid-run; requests in flight are drained first.
    pub async fn run(&self) -> Result<Stats> {
        let seeds = std::mem::take(&mut *lock(&self.seeds));
        for seed in seeds {
            self.frontier.push(seed).await?;
        }

        let started = Instant::now();
        *lock(&self.live) = Live {
            stats: Stats::default(),
            started: Some(started),
            running: true,
        };

        let settings = &self.settings;
        let deadline = settings.time_limit.map(|limit| started + limit);
        let mut run = self.new_run();
        tracing::info!(target: TRACING_TARGET, concurrency = settings.concurrency.get(), "crawl started");
        self.events.emit(|| Event::Started);

        // Timers and the shutdown signal are created once and re-armed as
        // needed: `select!` would otherwise build them again for every
        // finished request.
        let mut wait_timer = pin!(sleep_until(started));
        let mut armed_wait = None;
        let mut deadline_timer = pin!(sleep_until(deadline.unwrap_or(started)));
        let mut grace_timer = pin!(sleep_until(started));
        let mut armed_grace = None;
        let mut shutdown = pin!(self.shutdown.cancelled());

        loop {
            let wait = if run.stopping {
                self.resume_redirects(&mut run).await
            } else {
                self.fill(&mut run).await
            };

            if run.tasks.is_empty() && (run.stopping || (!run.robots_busy() && wait.is_none())) {
                break;
            }
            if let Some(at) = wait
                && armed_wait != wait
            {
                wait_timer.as_mut().reset(at);
                armed_wait = wait;
            }
            if let Some(at) = run.grace
                && armed_grace != run.grace
            {
                grace_timer.as_mut().reset(at);
                armed_grace = run.grace;
            }

            // Biased: the rare control branches are checked first, so a steady
            // stream of finished requests cannot delay a shutdown.
            tokio::select! {
                biased;
                () = &mut shutdown, if !run.stopping => {
                    run.stop(StopReason::Shutdown, settings.shutdown_grace);
                }
                () = &mut deadline_timer, if deadline.is_some() && !run.stopping => {
                    run.stop(StopReason::TimeLimit, settings.shutdown_grace);
                }
                () = &mut grace_timer, if run.grace.is_some() => {
                    tracing::warn!(target: TRACING_TARGET, in_flight = run.tasks.len(), "shutdown grace period over, cancelling requests");
                    run.grace = None;
                    run.tasks.abort_all();
                }
                Some(event) = run.redirects.recv() => self.transfer(&mut run, event).await,
                Some(joined) = run.tasks.join_next_with_id() => self.complete(&mut run, joined).await,
                fetched = next_robots(&mut run.robots) => self.robots_fetched(&mut run, fetched).await,
                () = &mut wait_timer, if wait.is_some() => {
                    armed_wait = None;
                }
            }
        }

        self.cancel_parked(&mut run).await;

        let stats = {
            let mut live = lock(&self.live);
            live.running = false;
            live.stats.elapsed = started.elapsed();
            live.stats
        };
        tracing::info!(
            target: TRACING_TARGET,
            requests = stats.requests,
            succeeded = stats.succeeded,
            failed = stats.failed,
            retried = stats.retried,
            skipped = stats.skipped,
            cancelled = stats.cancelled,
            elapsed_ms = stats.elapsed.as_millis(),
            "crawl finished",
        );
        self.events.emit(|| Event::Finished { stats });

        match run.error {
            Some(error) => Err(error),
            None => Ok(stats),
        }
    }

    async fn cancel_parked(&self, run: &mut Run<B>) {
        for pending in run.drain() {
            match pending {
                Pending::Seed(seed) => {
                    self.report(run, Report::Cancelled(seed)).await;
                }
                Pending::Redirect(redirect) => {
                    let _ = redirect.answer.send(Err(cancelled()));
                }
            }
        }
    }

    /// Sets up the state of a run: its identities, robots.txt and hosts.
    fn new_run(&self) -> Run<B> {
        let backend = self.backend.clone();
        let connector = Arc::new(match &self.identities {
            Some(identities) => Connector::with_identities(backend, identities.clone()),
            None => Connector::new(backend),
        });
        let robots = self.robots_run(&connector);
        // A robots.txt `Crawl-delay` may slow a host down, so hosts are
        // tracked whenever robots.txt is obeyed.
        let hosts = Hosts::new(self.settings.hosts, robots.is_some());
        let (redirect_sender, redirects) = mpsc::unbounded_channel();
        Run {
            tasks: JoinSet::new(),
            redirect_sender,
            redirects,
            in_flight: HashMap::new(),
            robots,
            hosts,
            connector,
            events: self.events.clone(),
            aborted: HashSet::new(),
            discovery: Discovery::new(self.settings.discovery_tag.clone()),
            started: 0,
            stopping: false,
            grace: None,
            error: None,
        }
    }

    /// Starts requests until the concurrency limit or budget is reached or
    /// no seed is ready, returning when the next one will be.
    async fn fill(&self, run: &mut Run<B>) -> Option<Instant> {
        let settings = &self.settings;
        let mut wait = run.robots_tick();
        self.resume_redirects(run).await;

        while run.tasks.len() < settings.concurrency.get() {
            if settings.max_requests.is_some_and(|max| run.started >= max) {
                run.stop(StopReason::RequestBudget, settings.shutdown_grace);
                return None;
            }

            if let Some(seed) = run.discovery.pop() {
                self.admit(run, seed.into()).await;
                continue;
            }
            if let Some(seed) = run.hosts.take_ready(Instant::now()) {
                self.start(run, seed).await;
                continue;
            }
            if run.parked() >= MAX_PARKED {
                break;
            }

            match self.frontier.pop_with(run.discovery.mode()).await {
                Ok(NextSeed::Ready(seed)) => self.admit(run, seed.into()).await,
                Ok(NextSeed::Wait(at)) => {
                    wait = wait.min(Some(at)).or(Some(at));
                    break;
                }
                Ok(NextSeed::Empty) => {
                    if run.tasks.is_empty()
                        && run.parked() == 0
                        && !run.robots_busy()
                        && run.discovery.finish()
                    {
                        continue;
                    }
                    break;
                }
                Err(error) => {
                    run.fail(error, settings.shutdown_grace);
                    return None;
                }
            }
        }

        let robots = run.robots_tick();
        [wait, robots, run.hosts.earliest()]
            .into_iter()
            .flatten()
            .min()
    }

    /// Redirect hops belong to already-running work, including during graceful
    /// shutdown. Resume them without starting new frontier requests.
    async fn resume_redirects(&self, run: &mut Run<B>) -> Option<Instant> {
        run.prune_admissions();
        while let Some(seed) = run
            .hosts
            .take_ready_where(Instant::now(), Pending::is_redirect)
        {
            self.start(run, seed).await;
        }
        let robots = run.robots_tick();
        let hosts = run.hosts.earliest_where(Pending::is_redirect);
        [robots, hosts].into_iter().flatten().min()
    }

    async fn transfer(&self, run: &mut Run<B>, event: TransferEvent) {
        match event {
            TransferEvent::Admit(request) => self.redirect(run, request).await,
            TransferEvent::Finished {
                task,
                started,
                latency,
            } => {
                if let Some(in_flight) = run.in_flight.get_mut(&task) {
                    in_flight.transfer = Some((started, Some(latency)));
                }
            }
        }
    }

    /// Moves a redirect through the same scope, robots and host admission as
    /// an ordinary seed, without changing depth or marking it as discovered.
    async fn redirect(&self, run: &mut Run<B>, redirect: RedirectRequest) {
        let RedirectRequest { task, seed, answer } = redirect;
        if answer.is_closed() {
            return;
        }
        let Some(in_flight) = run.in_flight.get_mut(&task) else {
            let _ = answer.send(Err(Error::invalid(
                "redirect was requested outside its crawler task",
            )));
            return;
        };
        // The first transfer uses the slot already admitted for this seed.
        // Every later transfer (including a fetch retried in one Context)
        // must re-enter host admission.
        if in_flight.transfer.is_none()
            && seed.uri() == in_flight.seed.uri()
            && seed.method() == in_flight.seed.method()
        {
            in_flight.transfer = Some((Instant::now(), None));
            let _ = answer.send(Ok(()));
            return;
        }
        if let HostSlot::Held(host) =
            std::mem::replace(&mut in_flight.host, HostSlot::WaitingForRedirect)
        {
            run.hosts.finished(&host, None, Instant::now());
        }
        if !self.frontier.permits(&seed) {
            let _ = answer.send(Err(Error::new(ErrorKind::Rejected)
                .with_message("redirect target is outside crawl scope")
                .with_uri(seed.uri().clone())));
            return;
        }
        self.admit(
            run,
            Pending::Redirect(RedirectRequest { task, seed, answer }),
        )
        .await;
    }

    /// Starts a popped seed now, parks it until robots.txt and its host
    /// allow it, or skips it if robots.txt disallows it.
    async fn admit(&self, run: &mut Run<B>, seed: Pending) {
        if run.stopping && !seed.is_redirect() {
            if let Pending::Seed(seed) = seed {
                self.report(run, Report::Cancelled(seed)).await;
            }
            return;
        }
        match run.robots_access(seed) {
            Access::Allow(seed) => match run.hosts.admit(seed, Instant::now()) {
                Admit::Start(seed) => self.start(run, seed).await,
                Admit::Parked => {}
            },
            Access::Disallow(seed) => self.skip(run, seed, SkipReason::Disallowed).await,
            Access::Parked => {}
        }
    }

    /// Builds the robots.txt state, fetching through the run's connector,
    /// as one of its identities if it has any, unless
    /// [`Robots`](crate::crawler::Robots) sets another backend.
    #[cfg(feature = "robots")]
    fn robots_run(&self, connector: &Arc<Connector<B>>) -> Option<RobotsRun> {
        let config = self.settings.robots.clone()?;
        let source = config
            .source
            .clone()
            .unwrap_or_else(|| Arc::new(BackendSource::new(connector.clone())));

        Some(RobotsRun {
            agent: config.agent.clone(),
            cache: RobotsCache::new(config),
            source,
            tasks: JoinSet::new(),
            fetches: HashMap::new(),
        })
    }

    #[cfg(not(feature = "robots"))]
    #[expect(clippy::unused_self, reason = "mirrors the signature with `robots`")]
    const fn robots_run(&self, _connector: &Arc<Connector<B>>) -> Option<RobotsRun> {
        None
    }

    /// Applies a fetched robots.txt and admits the seeds that waited for it.
    #[cfg(feature = "robots")]
    async fn robots_fetched(&self, run: &mut Run<B>, event: RobotsEvent) {
        let RobotsEvent::Fetched {
            task,
            origin,
            result,
        } = event
        else {
            return;
        };
        let Some(robots) = &mut run.robots else {
            return;
        };
        if robots
            .fetches
            .get(&origin)
            .is_none_or(|fetch| fetch.id() != task)
        {
            return;
        }
        robots.fetches.remove(&origin);
        run.events.emit(|| Event::RobotsFetched {
            origin: origin.clone(),
            error: result.as_ref().err().map(Failure::from),
        });
        let fetched = robots.cache.fetched(&origin, result, Instant::now());
        if let Some(delay) = fetched.delay {
            let host = origin
                .split_once("://")
                .map_or(origin.as_str(), |(_, host)| host);
            run.hosts.raise_delay(host, delay);
        }
        for seed in fetched.sitemaps {
            let uri = seed.uri().clone();
            if let Err(error) = self.queue().push(seed).await {
                tracing::warn!(target: TRACING_TARGET, %uri, %error, "failed to queue a sitemap");
            }
        }
        for seed in fetched.released {
            self.admit(run, seed).await;
        }
    }

    #[cfg(not(feature = "robots"))]
    #[expect(clippy::unused_self, reason = "mirrors the signature with `robots`")]
    fn robots_fetched(&self, _run: &mut Run<B>, never: RobotsEvent) -> std::future::Ready<()> {
        match never {}
    }

    /// Skips `seed` without crawling it.
    async fn skip(&self, run: &mut Run<B>, pending: Pending, reason: SkipReason) {
        let seed = match pending {
            Pending::Seed(seed) => seed,
            Pending::Redirect(redirect) => {
                let _ = redirect.answer.send(Err(Error::new(ErrorKind::Rejected)
                    .with_message(format!("redirect target skipped: {reason}"))
                    .with_uri(redirect.seed.uri().clone())));
                return;
            }
        };
        tracing::debug!(target: TRACING_TARGET, uri = %seed.uri(), %reason, "skipping request");
        self.tally(|stats| stats.skipped += 1);
        run.events.emit(|| Event::Skipped {
            request: (&seed).into(),
            reason,
        });
        self.report(run, Report::Skipped(seed)).await;
    }

    /// Skips `seed` if its tag was aborted, and otherwise spawns the request.
    async fn start(&self, run: &mut Run<B>, pending: Pending) {
        let seed = match pending {
            Pending::Seed(seed) => seed,
            Pending::Redirect(redirect) => {
                run.resume_redirect(redirect);
                return;
            }
        };
        let Some(seed) = run.discovery.admit(seed) else {
            return;
        };
        if run.is_aborted(&seed) {
            self.skip(run, seed.into(), SkipReason::TagAborted).await;
            return;
        }

        run.started += 1;
        let now = Instant::now();
        let host = run.hosts.start(&seed, now);
        run.events.emit(|| Event::RequestStarted {
            request: (&seed).into(),
        });
        let notify = run.events.notify(&seed);

        let worker = self.worker.clone();
        let queue = self.queue();
        let datasets = self.datasets.clone();
        let deadline = self.settings.request_timeout;
        let span = tracing::debug_span!(
            target: TRACING_TARGET,
            "request",
            method = %seed.method(),
            uri = %seed.uri(),
            tag = seed.tag().map(tracing::field::display),
            depth = %seed.depth(),
            attempt = %seed.attempt(),
        );

        // The identity is picked now, as the request starts, so a failure to
        // pick one fails the request like any other.
        let connector = run.connector.clone();
        let mut request = seed.clone().into_request();
        let mut extensions = self.extensions.clone();
        extensions.extend(std::mem::take(request.extensions_mut()));
        *request.extensions_mut() = extensions;
        let redirect_sender = run.redirect_sender.clone();
        let identity = connector.identify(&seed, &mut request);
        let used = identity.as_ref().ok().cloned().flatten();

        let task = async move {
            request.extensions_mut().insert(Redirects {
                sender: redirect_sender,
                task: tokio::task::id(),
            });
            let started = Instant::now();
            let handle = async {
                let connected = match identity {
                    Ok(identity) => connector.connect(identity.as_ref()).await,
                    Err(error) => Err(error),
                };
                let client = match connected {
                    Ok(client) => client,
                    Err(error) => return Outcome::Fail(error),
                };
                let queue: Queue = queue.child_of(&request);
                let cx = Context::from_parts(request, client, queue, datasets);
                worker.handle(cx).await
            };
            // The context records the fetch into this task's scope, which
            // outlives a timeout: a fetch that finished before the handlers
            // ran out of time still counts.
            let scope = FetchScope {
                fetch: Cell::default(),
                notify,
            };
            let (outcome, fetch) = FETCH
                .scope(scope, async {
                    let outcome = match deadline {
                        Some(deadline) => timeout(deadline, handle).await.unwrap_or_else(|_| {
                            let error =
                                Error::new(ErrorKind::Timeout).with_message("request timed out");
                            Outcome::Fail(error)
                        }),
                        None => handle.await,
                    };
                    (outcome, FETCH.with(|scope| scope.fetch.get()))
                })
                .await;

            tracing::debug!(
                target: TRACING_TARGET,
                outcome = %OutcomeKind::from(&outcome),
                elapsed_ms = started.elapsed().as_millis(),
                "request handled",
            );
            (outcome, fetch)
        };

        // Boxed so spawning moves a pointer, not the whole request future.
        let id = run.tasks.spawn(Box::pin(task.instrument(span))).id();
        let in_flight = InFlight {
            seed,
            destination: None,
            transfer: None,
            identity: used,
            host: HostSlot::active(host),
            started: now,
        };
        run.in_flight.insert(id, in_flight);
    }

    /// Records a finished request and reports its outcome to the frontier.
    async fn complete(&self, run: &mut Run<B>, joined: Result<(Id, (Outcome, Fetch)), JoinError>) {
        let (id, (outcome, fetch)) = match joined {
            Ok(joined) => joined,
            Err(error) if error.is_cancelled() => {
                let Some(InFlight { seed, host, .. }) = run.in_flight.remove(&error.id()) else {
                    return;
                };
                run.prune_admissions();
                if let HostSlot::Held(host) = host {
                    run.hosts.finished(&host, None, Instant::now());
                }
                self.tally(|stats| stats.cancelled += 1);
                run.events.emit(|| Event::Cancelled {
                    request: (&seed).into(),
                });
                self.report(run, Report::Cancelled(seed)).await;
                return;
            }
            Err(error) => {
                let failure = Error::worker("worker panicked").with_source(error.to_string());
                (error.id(), (Outcome::Fail(failure), Fetch::default()))
            }
        };
        let Some(InFlight {
            seed,
            destination,
            transfer,
            identity,
            host,
            started,
        }) = run.in_flight.remove(&id)
        else {
            return;
        };
        run.prune_admissions();
        // Built once for the events that follow, if anyone is subscribed.
        let request = run.events.is_active().then(|| RequestInfo::from(&seed));
        run.events.emit_for(request.as_ref(), |request| {
            handled(request, &outcome, started.elapsed())
        });

        let health = if matches!(host, HostSlot::WaitingForRedirect) {
            Health::Unknown
        } else {
            Health::of(&outcome, fetch.status)
        };
        let responding_seed = destination.as_deref().unwrap_or(&seed);
        let (host_started, host_latency) = transfer
            .map_or((started, fetch.latency), |(at, latency)| {
                (at, latency.or_else(|| Some(at.elapsed())))
            });
        let host_fetch = Fetch {
            latency: host_latency,
            ..fetch
        };
        match host {
            HostSlot::Held(key) => {
                run.finish_host(
                    Some(key),
                    responding_seed,
                    host_started,
                    host_fetch,
                    &outcome,
                );
            }
            HostSlot::Untracked => {
                run.finish_host(None, responding_seed, host_started, host_fetch, &outcome);
            }
            HostSlot::WaitingForRedirect => {}
        }
        if let Some(identity) = &identity {
            run.connector.finished(identity, health);
        }
        self.tally(|stats| {
            stats.requests += 1;
            if fetch.status == Some(StatusCode::NOT_MODIFIED) {
                stats.unchanged += 1;
            }
        });

        self.settle(run, seed, outcome, request).await;
    }

    /// Settles a handled request: applies what its outcome asks of the run,
    /// reports it to the frontier, and counts how it ended.
    async fn settle(
        &self,
        run: &mut Run<B>,
        seed: Seed,
        outcome: Outcome,
        request: Option<RequestInfo>,
    ) {
        let uri = seed.uri().clone();
        let mut failure = None;
        let mut failed = false;
        let report = match outcome {
            Outcome::Continue => {
                self.tally(|stats| stats.succeeded += 1);
                Report::Done(seed)
            }
            Outcome::Stop => {
                self.tally(|stats| stats.succeeded += 1);
                run.stop(StopReason::Requested, self.settings.shutdown_grace);
                Report::Done(seed)
            }
            Outcome::Abort(scope) => {
                let tag = match scope {
                    Scope::SameTag => seed.tag().cloned(),
                    Scope::Tag(tag) => Some(tag),
                };
                tracing::info!(target: TRACING_TARGET, tag = tag.as_ref().map(tracing::field::display), "aborting tag");
                run.aborted.insert(tag);
                self.tally(|stats| stats.succeeded += 1);
                Report::Done(seed)
            }
            Outcome::Retry { after } => Report::Retry { seed, after },
            Outcome::Backoff { scope, duration } => self.backoff(run, seed, scope, duration).await,
            Outcome::Fatal(error) => {
                failed = true;
                run.fail(error.with_uri(uri.clone()), self.settings.shutdown_grace);
                Report::Failed(seed)
            }
            Outcome::Fail(error) if error.kind() == ErrorKind::Frontier => {
                self.tally(|stats| stats.cancelled += 1);
                run.fail(error.with_uri(uri.clone()), self.settings.shutdown_grace);
                Report::Cancelled(seed)
            }
            Outcome::Fail(error) => {
                let error = match error.uri() {
                    Some(_) => error,
                    None => error.with_uri(uri.clone()),
                };
                if error.is_retryable() {
                    tracing::debug!(target: TRACING_TARGET, %uri, %error, "request failed, retrying");
                    // A `Retry-After` paused the host above; the request
                    // itself waits as long.
                    let after = error.retry_after();
                    failure = Some(error);
                    Report::Retry { seed, after }
                } else {
                    tracing::warn!(target: TRACING_TARGET, %uri, %error, "request failed");
                    failed = true;
                    Report::Failed(seed)
                }
            }
        };

        let retry = match &report {
            Report::Retry { after, .. } => Some(*after),
            _ => None,
        };
        match self.report(run, report).await {
            Some(Ack::Applied) if let Some(after) = retry => {
                self.tally(|stats| stats.retried += 1);
                run.events
                    .emit_for(request.as_ref(), |request| Event::Retried {
                        request,
                        after,
                    });
            }
            Some(Ack::RetryRefused) => {
                failed = true;
                tracing::warn!(
                    target: TRACING_TARGET,
                    %uri,
                    error = failure.as_ref().map(tracing::field::display),
                    "request failed, out of retries",
                );
            }
            _ => {}
        }
        if failed {
            self.tally(|stats| stats.failed += 1);
            run.events
                .emit_for(request.as_ref(), |request| Event::Failed { request });
        }
    }

    /// Pauses `scope` for `duration`, returning the report that retries
    /// `seed` once the pause ends.
    async fn backoff(
        &self,
        run: &mut Run<B>,
        seed: Seed,
        scope: Scope,
        duration: Duration,
    ) -> Report {
        let tag = match scope {
            Scope::SameTag => seed.tag().cloned(),
            Scope::Tag(tag) => Some(tag),
        };
        tracing::debug!(
            target: TRACING_TARGET,
            tag = tag.as_ref().map(tracing::field::display),
            duration_ms = duration.as_millis(),
            "backing off",
        );

        let until = Instant::now() + duration;
        self.report(run, Report::Backoff { tag, until }).await;
        Report::Retry {
            seed,
            after: Some(duration),
        }
    }

    /// Reports to the frontier; a failure stops the run.
    async fn report(&self, run: &mut Run<B>, report: Report) -> Option<Ack> {
        match self.frontier.report(report).await {
            Ok(ack) => Some(ack),
            Err(error) => {
                run.fail(error, self.settings.shutdown_grace);
                None
            }
        }
    }
}
