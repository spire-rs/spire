use std::convert::Infallible;
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::future::BoxFuture;
use http_body::Body;
use tower::{Layer, Service};

use crate::backend::util::Noop;
use crate::context::{Context as Cx, Request, Response, Signal, Task};
use crate::dataset::Dataset;
use crate::{Error, Result};

/// Tracing [`Backend`], [`Client`] and [`Worker`] middleware for improved observability.
#[derive(Clone)]
#[must_use = "services do nothing unless you `.poll_ready` or `.call` them"]
pub struct Trace<S> {
    inner: S,
    success_counter: Arc<AtomicUsize>,
    failure_counter: Arc<AtomicUsize>,
}

impl<S> Trace<S> {
    /// Creates a new [`Trace`].
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            success_counter: Arc::new(AtomicUsize::new(0)),
            failure_counter: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl Default for Trace<Noop> {
    #[inline]
    fn default() -> Self {
        Self::new(Noop::default())
    }
}

impl<T> fmt::Debug for Trace<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TraceWorker")
            .field("entity", &self.inner)
            .field("success", &self.success_counter.load(Ordering::SeqCst))
            .field("failure", &self.failure_counter.load(Ordering::SeqCst))
            .finish()
    }
}

impl<S> Service<()> for Trace<S>
where
    S: Service<(), Error = Error>,
    S::Response: Service<Request, Response = Response, Error = Error>,
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
        let fut = async {
            let client = self.inner.call(req).await?;
            tracing::trace!("initialized new client");
            Ok::<_, Error>(Trace::new(client))
        };

        todo!()
    }
}

impl<S> Service<Request> for Trace<S>
where
    S: Service<Request, Response = Response, Error = Error>,
{
    type Response = Response;
    type Error = Error;
    type Future = S::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, req: Request) -> Self::Future {
        let fut = async {
            tracing::trace!(
                lower = req.body().size_hint().lower(),
                upper = req.body().size_hint().upper(),
                "request body"
            );

            let resp = self.inner.call(req).await?;

            tracing::trace!(
                status = resp.status().as_u16(),
                lower = resp.body().size_hint().lower(),
                upper = resp.body().size_hint().upper(),
                "response body"
            );

            Ok::<_, Error>(resp)
        };

        todo!()
    }
}

impl<S, B> Service<Cx<B>> for Trace<S>
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
    fn call(&mut self, req: Cx<B>) -> Self::Future {
        let fut = async {
            let requests = req.dataset::<Request>().into_inner();

            tracing::trace!(
                depth = req.get_ref().depth(),
                requests = requests.len(),
                "invoked handler"
            );

            let signal = self.inner.call(req).await.unwrap();
            match &signal {
                Signal::Continue | Signal::Wait(..) => {
                    self.success_counter.fetch_add(1, Ordering::SeqCst);
                }
                Signal::Skip | Signal::Hold(..) | Signal::Fail(..) => {
                    self.failure_counter.fetch_add(1, Ordering::SeqCst);
                }
            };

            tracing::trace!(
                // signal = signal.as_str(),
                requests = requests.len(),
                "returned handler"
            );

            signal
        };

        todo!()
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
