use std::convert::Infallible;
use std::task::{Context, Poll};

use tower::load::Load;
use tower::{Layer, Service};

use crate::context::{Context as Cx, Signal};

#[derive(Debug, Default, PartialOrd, PartialEq, Clone)]
pub struct Stats {
    requests: usize,
    responses: usize,
}

#[derive(Debug, Clone)]
pub struct Metrics<S> {
    stats: Stats,
    inner: S,
}

impl<S> Metrics<S> {
    /// Creates a new [`Metrics`] service.
    pub fn new(inner: S, stats: Stats) -> Self {
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

impl<B, S> Service<Cx<B>> for Metrics<S>
where
    S: Service<Cx<B>, Response = Signal, Error = Infallible>,
{
    type Response = Signal;
    type Error = Infallible;
    type Future = S::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, cx: Cx<B>) -> Self::Future {
        self.inner.call(cx)
    }
}

impl<S> Load for Metrics<S> {
    type Metric = Stats;

    fn load(&self) -> Self::Metric {
        self.stats.clone()
    }
}

#[derive(Debug, Clone)]
pub struct MetricsLayer {
    stats: Stats,
}

impl MetricsLayer {
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
