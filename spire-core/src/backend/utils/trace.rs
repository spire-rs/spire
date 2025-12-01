use std::convert::Infallible;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::FutureExt;
use futures::future::BoxFuture;
use http_body::Body;
use pin_project_lite::pin_project;
use tower::{Layer, Service};

#[cfg(feature = "trace")]
use crate::TRACING_TARGET_BACKEND as TARGET;
use crate::context::{Context as Cx, Request, Response, Signal, Task};
use crate::dataset::Dataset;
use crate::{Error, Result};

/// Tracing middleware for [`Backend`], [`Client`], and [`Worker`] services.
///
/// `Trace` wraps services with `tracing` instrumentation to provide detailed logs
/// of operations during web scraping. It implements all three core traits and can
/// be composed with any compatible service.
///
/// # What Gets Traced
///
/// - **Backend**: Client initialization events
/// - **Client**: Request/response body sizes and HTTP status codes
/// - **Worker**: Request depth, queue size, and signal outcomes
///
/// # Requirements
///
/// This middleware requires the `trace` feature to be enabled.
///
/// # Examples
///
/// ## Wrapping Individual Services
///
/// ```ignore
/// use spire_core::backend::utils::Trace;
///
/// let backend = Trace::new(my_backend);
/// let worker = Trace::new(my_worker);
/// let client = Client::new(backend, worker);
/// ```
///
/// ## Using with Tower Layers
///
/// ```ignore
/// use spire_core::backend::utils::TraceLayer;
/// use tower::ServiceBuilder;
///
/// let backend = ServiceBuilder::new()
///     .layer(TraceLayer::new())
///     .service(my_backend);
/// ```
///
/// # Output Format
///
/// Trace events are emitted at the `TRACE` level with structured fields:
///
/// - Client init: `initialized new client`
/// - Request: `request body` with `lower` and `upper` size hints
/// - Response: `response body` with `status`, `lower`, and `upper` fields
/// - Worker: `handler requested/responded` with `depth` and `requests` count
///
/// [`Backend`]: crate::backend::Backend
/// [`Client`]: crate::backend::Client
/// [`Worker`]: crate::backend::Worker
#[derive(Clone)]
#[must_use = "services do nothing unless you `.poll_ready` or `.call` them"]
pub struct Trace<S> {
    inner: S,
}

impl<S> Trace<S> {
    /// Creates a new [`Trace`] middleware wrapping the given service.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_core::backend::utils::Trace;
    ///
    /// let traced = Trace::new(my_service);
    /// ```
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<T> fmt::Debug for Trace<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Trace").field("inner", &self.inner).finish()
    }
}

impl<S> Service<()> for Trace<S>
where
    S: Service<(), Error = Error> + Clone + Send + 'static,
    S::Response: Service<Request, Response = Response, Error = Error>,
    S::Future: Send + 'static,
{
    type Error = S::Error;
    type Future = TraceFuture<Trace<S::Response>, S::Error>;
    type Response = Trace<S::Response>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, req: ()) -> Self::Future {
        let mut inner = self.inner.clone();
        let fut = async move {
            let client = inner.call(req).await?;
            tracing::trace!(target: TARGET, "initialized new client");
            Ok::<_, Error>(Trace::new(client))
        };

        TraceFuture::new(fut.boxed())
    }
}

impl<S> Service<Request> for Trace<S>
where
    S: Service<Request, Response = Response, Error = Error> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Error = Error;
    type Future = TraceFuture<S::Response, S::Error>;
    type Response = Response;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, req: Request) -> Self::Future {
        let mut inner = self.inner.clone();
        let fut = async move {
            tracing::trace!(
                target: TARGET,
                lower = req.body().size_hint().lower(),
                upper = req.body().size_hint().upper(),
                "request body"
            );

            let resp = inner.call(req).await?;
            tracing::trace!(
                target: TARGET,
                status = resp.status().as_u16(),
                lower = resp.body().size_hint().lower(),
                upper = resp.body().size_hint().upper(),
                "response body"
            );

            Ok::<_, Error>(resp)
        };

        TraceFuture::new(fut.boxed())
    }
}

impl<S, C> Service<Cx<C>> for Trace<S>
where
    S: Service<Cx<C>, Response = Signal, Error = Infallible> + Clone + Send + 'static,
    C: Service<Request, Response = Response, Error = Error> + Send + 'static,
    S::Future: Send + 'static,
{
    type Error = Infallible;
    type Future = TraceFuture<S::Response, S::Error>;
    type Response = Signal;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, cx: Cx<C>) -> Self::Future {
        let mut inner = self.inner.clone();

        let fut = async move {
            let dataset = cx.dataset::<Request>();
            let requests = dataset.as_dataset();
            tracing::trace!(
                target: TARGET,
                depth = cx.get_ref().depth(),
                requests = requests.len(),
                "handler requested"
            );

            let signal = inner.call(cx).await;
            tracing::trace!(
                target: TARGET,
                // signal = signal.as_str(),
                requests = requests.len(),
                "handler responded"
            );

            signal
        };

        TraceFuture::new(fut.boxed())
    }
}

pin_project! {
    /// Response [`Future`] for [`Trace`] middleware.
    ///
    /// This future wraps the inner service's future and is returned by
    /// [`Trace::call`]. It transparently polls the wrapped future without
    /// adding additional tracing overhead.
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct TraceFuture<T, E> {
        #[pin] fut: BoxFuture<'static, Result<T, E>>,
    }
}

impl<T, E> TraceFuture<T, E> {
    /// Creates a new [`TraceFuture`].
    #[inline]
    fn new(fut: BoxFuture<'static, Result<T, E>>) -> Self {
        Self { fut }
    }
}

impl<T, E> Future for TraceFuture<T, E> {
    type Output = Result<T, E>;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        this.fut.poll(cx)
    }
}

/// A `tower::`[`Layer`] that produces a [`Trace`] service.
///
/// `TraceLayer` implements Tower's [`Layer`] trait, making it composable with
/// other layers in a [`ServiceBuilder`] chain.
///
/// # Examples
///
/// ```ignore
/// use spire_core::backend::utils::TraceLayer;
/// use tower::ServiceBuilder;
///
/// let service = ServiceBuilder::new()
///     .layer(TraceLayer::new())
///     .service(my_backend);
/// ```
///
/// [`ServiceBuilder`]: tower::ServiceBuilder
#[derive(Debug, Default, Clone)]
#[must_use = "layers do nothing unless you `.layer` them"]
pub struct TraceLayer {}

impl TraceLayer {
    /// Creates a new [`TraceLayer`].
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_core::backend::utils::TraceLayer;
    ///
    /// let layer = TraceLayer::new();
    /// ```
    pub fn new() -> Self {
        Self::default()
    }
}

impl<S> Layer<S> for TraceLayer {
    type Service = Trace<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Trace::new(inner)
    }
}
