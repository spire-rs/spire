//! The [`Crawler`], which drives a crawl to completion.

mod concurrency;
mod connector;
mod delay;
pub mod event;
mod hosts;
mod meta;
#[cfg(feature = "robots")]
mod robots;
mod run;
mod stats;
#[cfg(test)]
mod tests;

use std::fmt;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub use concurrency::HostConcurrency;
pub use delay::HostDelay;
pub use meta::{Attempt, Depth, Revisit, Tag, Tags};
#[cfg(feature = "robots")]
#[cfg_attr(docsrs, doc(cfg(feature = "robots")))]
pub use robots::Robots;
pub use stats::Stats;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::Result;
use crate::backend::Backend;
use crate::crawler::event::{Emitter, Events};
use crate::crawler::hosts::HostPolicy;
use crate::dataset::{Data, Dataset, Datasets};
use crate::frontier::dynamic::DynFrontier;
use crate::frontier::facade::RetryPolicy;
use crate::frontier::queue::Queue;
use crate::frontier::seed::Seed;
use crate::frontier::{Frontier, FrontierExt, InMemFrontier};
use crate::identity::Identities;
use crate::sync::lock;
use crate::worker::Worker;

/// The default number of requests handled at once.
const DEFAULT_CONCURRENCY: NonZeroUsize = NonZeroUsize::new(8).unwrap();

/// Crawls requests from a [`Frontier`] with a [`Backend`] and a [`Worker`].
///
/// [`run`](Self::run) pops seeds from the frontier and handles up to
/// [`with_concurrency`](Self::with_concurrency) of them at once. Each one
/// gets its own client from the backend and is handed to the worker, whose
/// [`Outcome`] is reported back to the frontier.
///
/// The run ends once the frontier is empty and no request is in flight, when
/// a worker returns [`Outcome::Stop`], the request or time budget is spent,
/// or the [`shutdown_token`](Self::shutdown_token) is cancelled. Stopping
/// lets requests in flight finish, for at most the
/// [shutdown grace period](Self::with_shutdown_grace) if one is set.
///
/// Politeness is per host: [`with_host_concurrency`](Self::with_host_concurrency)
/// and [`with_host_delay`](Self::with_host_delay) limit how hard a single
/// host is crawled, and with the `robots` feature, `with_robots` obeys
/// robots.txt.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
///
/// use spire_core::backend::Noop;
/// use spire_core::frontier::seed::Seed;
/// use spire_core::frontier::{FrontierExt, InMemFrontier};
/// use spire_core::worker::from_fn;
/// use spire_core::crawler::Crawler;
/// use spire_core::{Context, Result};
///
/// async fn handle(cx: Context<Noop>) -> Result<()> {
///     cx.queue().follow("https://example.com/next").await?;
///     Ok(())
/// }
///
/// # async fn example() -> Result<()> {
/// let crawler = Crawler::new(Noop, from_fn(handle))
///     .with_frontier(InMemFrontier::fifo().dedup().allow_hosts(["example.com"]))
///     .with_max_requests(100)
///     .with_host_concurrency(2)
///     .with_request_timeout(Duration::from_secs(30))
///     .with_seed(Seed::get("https://example.com/")?);
///
/// let stats = crawler.run().await?;
/// assert_eq!(stats.requests, 2);
/// # Ok(())
/// # }
/// ```
///
/// [`Backend`]: crate::backend::Backend
/// [`Worker`]: crate::worker::Worker
/// [`Outcome`]: crate::worker::Outcome
/// [`Outcome::Stop`]: crate::worker::Outcome::Stop
pub struct Crawler<B, W> {
    backend: Arc<B>,
    /// The identities requests go out as, if not directly.
    identities: Option<Identities>,
    worker: Arc<W>,
    frontier: Arc<dyn DynFrontier>,
    datasets: Arc<Datasets>,
    extensions: http::Extensions,
    seeds: Mutex<Vec<Seed>>,
    settings: Settings,
    shutdown: CancellationToken,
    live: Arc<Mutex<Live>>,
    events: Emitter,
}

/// How a run behaves.
#[derive(Debug, Clone)]
struct Settings {
    concurrency: NonZeroUsize,
    max_requests: Option<usize>,
    time_limit: Option<Duration>,
    request_timeout: Option<Duration>,
    shutdown_grace: Option<Duration>,
    hosts: HostPolicy,
    #[cfg(feature = "robots")]
    robots: Option<Robots>,
}

/// The stats of the current or last run.
#[derive(Debug, Default)]
struct Live {
    stats: Stats,
    started: Option<Instant>,
    running: bool,
}

impl<B, W> Crawler<B, W> {
    /// Creates a crawler with an [`InMemFrontier::fifo`] frontier that
    /// retries with the default [`RetryPolicy`], no datasets, a concurrency
    /// of 8 and no limits.
    ///
    /// `worker` must handle the clients of `backend`, which also lets the
    /// compiler infer a router's client type from the backend.
    pub fn new(backend: B, worker: W) -> Self
    where
        B: Backend,
        W: Worker<B::Session>,
    {
        let frontier = InMemFrontier::fifo().retry_with(RetryPolicy::default());
        let settings = Settings {
            concurrency: DEFAULT_CONCURRENCY,
            max_requests: None,
            time_limit: None,
            request_timeout: None,
            shutdown_grace: None,
            hosts: HostPolicy::default(),
            #[cfg(feature = "robots")]
            robots: None,
        };

        Self {
            backend: Arc::new(backend),
            identities: None,
            worker: Arc::new(worker),
            frontier: Arc::new(frontier),
            datasets: Arc::default(),
            extensions: http::Extensions::new(),
            seeds: Mutex::default(),
            settings,
            shutdown: CancellationToken::new(),
            live: Arc::default(),
            events: Emitter::new(event::CAPACITY),
        }
    }

    /// Adds a shared service to every request handled by this crawler.
    ///
    /// A later registration of the same type replaces the previous value.
    /// Values are cloned for each request; use `Arc<T>` for shared ownership.
    /// Middleware can replace a value on an individual request.
    #[must_use]
    pub fn with_extension<T: Clone + Send + Sync + 'static>(mut self, value: T) -> Self {
        self.extensions.insert(value);
        self
    }

    /// Replaces the frontier.
    ///
    /// Seeds added with [`with_seed`](Self::with_seed) are pushed when the
    /// run starts, so they go to the new frontier.
    #[must_use]
    pub fn with_frontier(mut self, frontier: impl Frontier) -> Self {
        self.frontier = Arc::new(frontier);
        self
    }

    /// Registers `dataset` for items of type `T`, replacing any previous one.
    #[must_use]
    pub fn with_dataset<T, D>(mut self, dataset: D) -> Self
    where
        T: Send + 'static,
        D: Dataset<T>,
    {
        Arc::make_mut(&mut self.datasets).insert(dataset);
        self
    }

    /// Sets how many requests are handled at once, at least one.
    #[must_use]
    pub fn with_concurrency(mut self, limit: usize) -> Self {
        self.settings.concurrency = NonZeroUsize::new(limit).unwrap_or(NonZeroUsize::MIN);
        self
    }

    /// Stops the run after starting `max` requests, counting every attempt.
    #[must_use]
    pub const fn with_max_requests(mut self, max: usize) -> Self {
        self.settings.max_requests = Some(max);
        self
    }

    /// Stops the run once it has lasted `limit`.
    #[must_use]
    pub const fn with_time_limit(mut self, limit: Duration) -> Self {
        self.settings.time_limit = Some(limit);
        self
    }

    /// Fails a request that takes longer than `timeout` to handle, fetching
    /// included. Timed out requests are retried.
    #[must_use]
    pub const fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.settings.request_timeout = Some(timeout);
        self
    }

    /// Cancels requests still in flight `grace` after the run starts
    /// stopping, instead of waiting for them indefinitely.
    ///
    /// Cancelled requests are reported to the frontier as
    /// [`Report::Cancelled`](crate::frontier::Report::Cancelled).
    #[must_use]
    pub const fn with_shutdown_grace(mut self, grace: Duration) -> Self {
        self.settings.shutdown_grace = Some(grace);
        self
    }

    /// Sets how many requests to the same host are in flight at once:
    /// a fixed number, at least one, or a limit per host adapted to how it
    /// responds with [`HostConcurrency::adaptive`]. Unlimited by default.
    #[must_use]
    pub fn with_host_concurrency(mut self, limit: impl Into<HostConcurrency>) -> Self {
        self.settings.hosts.concurrency = Some(limit.into());
        self
    }

    /// Sets the gap between starting two requests to the same host: at
    /// least a minimum, a [`Duration`], growing while the host is throttled
    /// up to a maximum, one minute unless a range sets it; see
    /// [`HostDelay`]. No minimum by default.
    #[must_use]
    pub fn with_host_delay(mut self, delay: impl Into<HostDelay>) -> Self {
        self.settings.hosts.delay = delay.into();
        self
    }

    /// Obeys robots.txt: fetches each origin's file before crawling it,
    /// skips what it disallows, and honours its `Crawl-delay`.
    ///
    /// Takes a user agent, or a [`Robots`] to fetch with another backend or
    /// change how long rules are kept; see [`Robots`] for how failures are
    /// handled. Disallowed requests are reported as
    /// [`Report::Skipped`](crate::frontier::Report::Skipped).
    ///
    /// ```
    /// use spire_core::backend::Noop;
    /// use spire_core::crawler::Robots;
    /// use spire_core::worker::from_fn;
    /// use spire_core::crawler::Crawler;
    /// use spire_core::Context;
    ///
    /// let worker = from_fn(|_: Context<Noop>| async {});
    /// let crawler = Crawler::new(Noop, worker).with_robots("MyBot");
    ///
    /// // robots.txt through another backend, e.g. HTTP for a browser crawl.
    /// let worker = from_fn(|_: Context<Noop>| async {});
    /// let crawler = Crawler::new(Noop, worker).with_robots(Robots::new("MyBot").with_backend(Noop));
    /// ```
    #[cfg(feature = "robots")]
    #[cfg_attr(docsrs, doc(cfg(feature = "robots")))]
    #[must_use]
    pub fn with_robots(mut self, robots: impl Into<Robots>) -> Self {
        self.settings.robots = Some(robots.into());
        self
    }

    /// Adds a seed to push when the run starts.
    #[must_use]
    pub fn with_seed(self, seed: Seed) -> Self {
        lock(&self.seeds).push(seed);
        self
    }

    /// Adds seeds to push when the run starts.
    #[must_use]
    pub fn with_seeds(self, seeds: impl IntoIterator<Item = Seed>) -> Self {
        lock(&self.seeds).extend(seeds);
        self
    }

    /// Sends every request as one of `identities`: through its proxy, with
    /// its headers and its own cookies, so each looks like one visitor.
    /// robots.txt is fetched as an identity too, unless robots.txt has a
    /// backend of its own.
    ///
    /// The backend connects each request as its identity, see
    /// [`Backend::connect_as`]; one that cannot route through a proxy fails
    /// the requests of identities that have one. Identities that keep
    /// failing are taken out of use, see [`Identities`].
    ///
    /// ```
    /// use spire_core::backend::Noop;
    /// use spire_core::identity::{Identities, Identity};
    /// use spire_core::worker::from_fn;
    /// use spire_core::crawler::Crawler;
    /// use spire_core::Context;
    ///
    /// let identities = Identities::new([Identity::new("a"), Identity::new("b")]);
    /// let worker = from_fn(|_: Context<Noop>| async {});
    /// let crawler = Crawler::new(Noop, worker).with_identities(identities);
    /// ```
    #[must_use]
    pub fn with_identities(mut self, identities: Identities) -> Self {
        self.identities = Some(identities);
        self
    }

    /// Returns the backend, e.g. to shut it down after a run.
    #[must_use]
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Returns a queue for adding seeds to the frontier.
    #[must_use]
    pub fn queue(&self) -> Queue {
        Queue::from_dyn(self.frontier.clone())
    }

    /// Returns the dataset for items of type `T`.
    ///
    /// # Errors
    ///
    /// Returns a [`Dataset`](crate::ErrorKind::Dataset) error if no dataset
    /// is registered for `T`.
    pub fn dataset<T: Send + 'static>(&self) -> Result<Data<T>> {
        self.datasets.get()
    }

    /// Returns a copy of every seed the frontier holds, see
    /// [`Frontier::snapshot`]. After a stopped run it includes the requests
    /// cancelled or never started.
    #[must_use]
    pub fn snapshot(&self) -> Option<Vec<Seed>> {
        self.frontier.snapshot()
    }

    /// Returns the stats of the current run while it runs, or of the last
    /// one.
    #[must_use]
    pub fn stats(&self) -> Stats {
        let live = lock(&self.live);
        let mut stats = live.stats;
        if live.running
            && let Some(started) = live.started
        {
            stats.elapsed = started.elapsed();
        }
        stats
    }

    /// Subscribes to the [`Event`]s of every run from now on: requests
    /// starting, fetched, handled, retried or skipped, hosts slowed down,
    /// robots.txt fetched, and the run stopping and finishing.
    ///
    /// Events are not built while no one is subscribed. A subscriber never
    /// slows the crawl down: one that falls more than 1024 events behind
    /// misses the oldest, and gets an [`Event::Lagged`] in their place. The
    /// stream ends once the crawler is dropped; [`Event::Finished`] marks
    /// the end of a run.
    ///
    /// ```
    /// use futures::StreamExt;
    /// use spire_core::backend::Noop;
    /// use spire_core::crawler::Crawler;
    /// use spire_core::crawler::event::Event;
    /// use spire_core::frontier::seed::Seed;
    /// use spire_core::worker::from_fn;
    /// use spire_core::{Context, Result};
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<()> {
    /// let worker = from_fn(|_: Context<Noop>| async {});
    /// let crawler = Crawler::new(Noop, worker).with_seed(Seed::get("https://example.com/")?);
    ///
    /// let mut events = crawler.events();
    /// let watch = tokio::spawn(async move {
    ///     let mut handled = 0;
    ///     while let Some(event) = events.next().await {
    ///         match event {
    ///             Event::Handled { .. } => handled += 1,
    ///             Event::Finished { .. } => break,
    ///             _ => {}
    ///         }
    ///     }
    ///     handled
    /// });
    ///
    /// crawler.run().await?;
    /// assert_eq!(watch.await.unwrap(), 1);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`Event`]: event::Event
    /// [`Event::Lagged`]: event::Event::Lagged
    /// [`Event::Finished`]: event::Event::Finished
    pub fn events(&self) -> Events {
        self.events.subscribe()
    }

    /// Returns a token that stops the run when cancelled.
    ///
    /// Requests in flight finish; nothing new starts.
    #[must_use]
    pub fn shutdown_token(&self) -> CancellationToken {
        self.shutdown.clone()
    }

    /// Updates the live stats.
    fn tally(&self, update: impl FnOnce(&mut Stats)) {
        update(&mut lock(&self.live).stats);
    }
}

impl<B, W> fmt::Debug for Crawler<B, W> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Crawler")
            .field("settings", &self.settings)
            .field("identities", &self.identities)
            .field("queued", &self.frontier.size_hint())
            .field("datasets", &self.datasets)
            .finish_non_exhaustive()
    }
}
