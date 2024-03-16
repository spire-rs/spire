use std::convert::Infallible;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{ready, Context, Poll};
use std::time::{Duration, Instant};

use pin_project_lite::pin_project;
use tower::load::Load;
use tower::{Layer, Service};

use crate::context::{Context as Cx, Signal};

/// TODO: Serde.
/// sequential estimation
#[derive(Debug, Default, PartialOrd, PartialEq, Clone)]
pub struct Stats {
    pub requests: u32,
    pub responses: u32,
    // pub failures: u32,
    pub average: Duration,
}

/// TODO.
#[derive(Clone)]
pub(crate) struct StatsLock {
    stats: Arc<Mutex<Stats>>,
}

impl StatsLock {
    pub fn notify_request(&self) {
        let mut guard = self.stats.lock().unwrap();
        guard.requests += 1;
    }

    pub fn notify_response(&self, since: Duration) {
        let mut guard = self.stats.lock().unwrap();
        guard.responses += 1;

        let prev_total = guard.average.as_millis() * guard.requests as u128;
        let curr_total = prev_total + since.as_millis();
        let average = curr_total / guard.requests as u128 + 1u128;

        guard.average = Duration::from_millis(average as u64);
    }
}

/// TODO.
#[derive(Clone)]
pub struct Metrics<S> {
    stats: StatsLock,
    inner: S,
}

impl<S> Metrics<S> {
    /// Creates a new [`Metrics`] service.
    pub fn new(inner: S, stats: Stats) -> Self {
        let stats = StatsLock {
            stats: Arc::new(Mutex::new(stats)),
        };

        Self { inner, stats }
    }

    /// Returns a reference to the inner service.
    pub fn get_ref(&self) -> &S {
        &self.inner
    }

    /// Returns a mutable reference to the inner service.
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.inner
    }

    /// Returns the inner service, consuming self.
    pub fn into_inner(self) -> S {
        self.inner
    }
}

impl<S> fmt::Debug for Metrics<S>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

impl<B, S> Service<Cx<B>> for Metrics<S>
where
    S: Service<Cx<B>, Response = Signal, Error = Infallible>,
{
    type Response = Signal;
    type Error = Infallible;
    type Future = MetricsFuture<S::Future>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, cx: Cx<B>) -> Self::Future {
        let stats = StatsLock {
            stats: self.stats.stats.clone(),
        };

        MetricsFuture::new(self.inner.call(cx), stats)
    }
}

impl<S> Load for Metrics<S> {
    type Metric = Stats;

    fn load(&self) -> Self::Metric {
        let guard = self.stats.stats.lock().unwrap();
        guard.clone()
    }
}

pin_project! {
    /// Response [`Future`] for [`Metrics`].
    pub struct MetricsFuture<F> {
        #[pin] future: F,
        created: Instant,
        stats: StatsLock,
    }
}

impl<F> MetricsFuture<F> {
    /// Creates a new [`MetricsFuture`].
    pub(crate) fn new(future: F, stats: StatsLock) -> Self {
        let created = Instant::now();
        stats.notify_request();
        Self {
            future,
            created,
            stats,
        }
    }
}

impl<F> Future for MetricsFuture<F>
where
    F: Future<Output = Result<Signal, Infallible>>,
{
    type Output = Result<Signal, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let signal = ready!(this.future.poll(cx));
        let since = Instant::now().duration_since(*this.created);
        this.stats.notify_response(since);

        Poll::Ready(signal)
    }
}

/// TODO.
#[derive(Debug, Clone)]
pub struct MetricsLayer {
    stats: Stats,
}

impl MetricsLayer {
    /// Creates a new [`MetricsLayer`].
    pub fn new(stats: Stats) -> Self {
        Self { stats }
    }
}

impl Default for MetricsLayer {
    fn default() -> Self {
        Self::new(Stats::default())
    }
}

impl<S> Layer<S> for MetricsLayer {
    type Service = Metrics<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Metrics::new(inner, self.stats.clone())
    }
}
