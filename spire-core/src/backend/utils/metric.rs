use std::convert::Infallible;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};

use futures::FutureExt;
use futures::future::BoxFuture;
use pin_project_lite::pin_project;
use tower::load::Load;
use tower::{Layer, Service};

use crate::context::{Context as Cx, Request, Response, Signal};
use crate::{Error, Result};

/// Metric collection middleware for [`Worker`] services.
///
/// `Metric` wraps worker services to collect performance metrics including success
/// and failure counts. It implements Tower's [`Load`] trait, making it compatible
/// with load balancing strategies.
///
/// # Collected Metrics
///
/// - **Success count**: Number of successful worker invocations (Continue/Skip signals)
/// - **Failure count**: Number of failed worker invocations (errors or Stop signals)
///
/// # Requirements
///
/// This middleware requires the `metric` feature to be enabled.
///
/// # Examples
///
/// ## Basic Usage
///
/// ```ignore
/// use spire_core::backend::utils::Metric;
///
/// let worker = Metric::new(my_worker);
///
/// // After processing requests
/// let load = worker.load();
/// println!("Worker load: {}", load);
/// ```
///
/// ## Using with Tower Layers
///
/// ```ignore
/// use spire_core::backend::utils::MetricLayer;
/// use tower::ServiceBuilder;
///
/// let worker = ServiceBuilder::new()
///     .layer(MetricLayer::new())
///     .service(my_worker);
/// ```
///
/// # Load Calculation
///
/// The load is calculated as the total number of failures, making workers with
/// fewer failures appear less loaded to load balancers.
///
/// [`Worker`]: crate::backend::Worker
#[derive(Clone)]
#[must_use = "services do nothing unless you `.poll_ready` or `.call` them"]
pub struct Metric<S> {
    inner: S,
    metrics: Arc<MetricInner>,
}

#[derive(Clone, Default)]
struct MetricInner {
    success: Arc<AtomicU64>,
    failure: Arc<AtomicU64>,
}

impl<S> Metric<S> {
    /// Creates a new [`Metric`] middleware wrapping the given worker.
    ///
    /// Initializes success and failure counters to zero.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_core::backend::utils::Metric;
    ///
    /// let worker = Metric::new(my_worker);
    /// ```
    pub fn new(inner: S) -> Self {
        let metrics = Arc::new(MetricInner::default());
        Self { inner, metrics }
    }
}

impl<T> fmt::Debug for Metric<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let success = self.metrics.success.load(Ordering::SeqCst);
        let failure = self.metrics.failure.load(Ordering::SeqCst);

        f.debug_struct("Metric")
            .field("inner", &self.inner)
            .field("success", &success)
            .field("failure", &failure)
            .finish()
    }
}

impl<S, C> Service<Cx<C>> for Metric<S>
where
    S: Service<Cx<C>, Response = Signal, Error = Infallible> + Clone + Send + 'static,
    C: Service<Request, Response = Response, Error = Error> + Send + 'static,
    S::Future: Send + 'static,
{
    type Error = Infallible;
    type Future = MetricFuture;
    type Response = Signal;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, cx: Cx<C>) -> Self::Future {
        let mut inner = self.inner.clone();
        let metrics = self.metrics.clone();
        let fut = async move {
            let signal = inner.call(cx).await.expect("should be infallible");
            match &signal {
                Signal::Continue | Signal::Wait(..) => {
                    metrics.success.fetch_add(1, Ordering::SeqCst);
                }
                Signal::Skip | Signal::Hold(..) | Signal::Fail(..) => {
                    metrics.failure.fetch_add(1, Ordering::SeqCst);
                }
            };

            Ok(signal)
        };

        MetricFuture::new(fut.boxed())
    }
}

impl<S> Load for Metric<S> {
    type Metric = u64;

    fn load(&self) -> Self::Metric {
        let success = self.metrics.success.load(Ordering::SeqCst);
        let failure = self.metrics.failure.load(Ordering::SeqCst);
        success - failure
    }
}

pin_project! {
    /// Response [`Future`] for [`Metric`] middleware.
    ///
    /// This future wraps the worker's future and updates metrics based on
    /// the returned signal when polled to completion.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct MetricFuture {
        #[pin] fut: BoxFuture<'static, Result<Signal, Infallible>>,
    }
}

impl MetricFuture {
    /// Creates a new [`MetricFuture`].
    #[inline]
    fn new(fut: BoxFuture<'static, Result<Signal, Infallible>>) -> Self {
        Self { fut }
    }
}

impl Future for MetricFuture {
    type Output = Result<Signal, Infallible>;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        this.fut.poll(cx)
    }
}

/// A `tower::`[`Layer`] that produces a [`Metric`] service.
///
/// `MetricLayer` implements Tower's [`Layer`] trait for composing metric
/// collection into service builder chains.
///
/// # Examples
///
/// ```ignore
/// use spire_core::backend::utils::MetricLayer;
/// use tower::ServiceBuilder;
///
/// let worker = ServiceBuilder::new()
///     .layer(MetricLayer::new())
///     .service(my_worker);
/// ```
///
/// [`ServiceBuilder`]: tower::ServiceBuilder
#[derive(Debug, Default, Clone)]
#[must_use = "layers do nothing unless you `.layer` them"]
pub struct MetricLayer {}

impl MetricLayer {
    /// Creates a new [`MetricLayer`].
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_core::backend::utils::MetricLayer;
    ///
    /// let layer = MetricLayer::new();
    /// ```
    pub fn new() -> Self {
        Self::default()
    }
}

impl<S> Layer<S> for MetricLayer {
    type Service = Metric<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Metric::new(inner)
    }
}
