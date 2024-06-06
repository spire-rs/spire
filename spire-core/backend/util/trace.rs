use std::convert::Infallible;
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::future::BoxFuture;
use futures::{FutureExt, StreamExt};
use http_body::Body;
use tower::{Layer, Service, ServiceExt};

use crate::backend::util::Noop;
use crate::context::{Context as Cx, Request, Response, Signal, Task};
use crate::dataset::Dataset;
use crate::{Error, Result};

/// Tracing [`Backend`], [`Client`] and [`Worker`] middleware for improved observability.
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
    type Future = BoxFuture<'static, Result<Trace<S::Response>, S::Error>>;

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

        fut.boxed()
    }
}

impl<S> Service<Request> for Trace<S>
where
    S: Service<Request, Response = Response, Error = Error> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = Error;
    type Future = BoxFuture<'static, Result<S::Response, S::Error>>;

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

        fut.boxed()
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
    type Future = BoxFuture<'static, Result<Signal, Infallible>>;

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

        fut.boxed()
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
