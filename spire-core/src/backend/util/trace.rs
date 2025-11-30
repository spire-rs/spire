use std::convert::Infallible;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::future::BoxFuture;
use futures::FutureExt;
use http_body::Body;
use pin_project_lite::pin_project;
use tower::{Layer, Service};

use crate::context::{Context as Cx, Request, Response, Signal, Task};
use crate::dataset::Dataset;
use crate::{Error, Result};

/// Tracing `tower::`[`Service`] for improved observability.
///
/// Supports [`Backend`], [`Client`] and [`Worker`].
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
    /// Creates a new [`Trace`].
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
    type Response = Trace<S::Response>;
    type Error = S::Error;
    type Future = TraceFuture<Trace<S::Response>, S::Error>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, req: ()) -> Self::Future {
        let mut inner = self.inner.clone();
        let fut = async move {
            let client = inner.call(req).await?;
            tracing::trace!("initialized new client");
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
    type Response = Response;
    type Error = Error;
    type Future = TraceFuture<S::Response, S::Error>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, req: Request) -> Self::Future {
        let mut inner = self.inner.clone();
        let fut = async move {
            tracing::trace!(
                lower = req.body().size_hint().lower(),
                upper = req.body().size_hint().upper(),
                "request body"
            );

            let resp = inner.call(req).await?;
            tracing::trace!(
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
    type Response = Signal;
    type Error = Infallible;
    type Future = TraceFuture<S::Response, S::Error>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, cx: Cx<C>) -> Self::Future {
        let mut inner = self.inner.clone();

        let fut = async move {
            let requests = cx.dataset::<Request>().into_inner();
            tracing::trace!(
                depth = cx.get_ref().depth(),
                requests = requests.len(),
                "handler requested"
            );

            let signal = inner.call(cx).await;
            tracing::trace!(
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
    /// Response [`Future`] for [`Trace`].
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
#[derive(Debug, Default, Clone)]
#[must_use = "layers do nothing unless you `.layer` them"]
pub struct TraceLayer {}

impl TraceLayer {
    /// Creates a new [`TraceLayer`].
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
