use std::convert::Infallible;
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::FutureExt;
use futures::future::Map;
use pin_project_lite::pin_project;
use tower::Service;

use crate::context::{Context as Cx, FlowControl};
use crate::handler::Handler;

/// Implementation of a `tower::`[`Service`] for a [`Handler`].
///
/// Automatically implements [`Worker`].
///
/// [`Worker`]: crate::backend::Worker
#[must_use = "services do nothing unless you `.poll_ready` or `.call` them"]
pub struct HandlerService<H, V, S> {
    marker: PhantomData<V>,
    handler: H,
    state: S,
}

impl<H, V, S> HandlerService<H, V, S> {
    /// Creates a new [`HandlerService`].
    pub const fn new<C>(handler: H, state: S) -> Self
    where
        H: Handler<C, V, S>,
    {
        Self {
            marker: PhantomData,
            handler,
            state,
        }
    }

    /// Gets a reference to the state.
    #[inline]
    pub const fn state_ref(&self) -> &S {
        &self.state
    }

    /// Gets a mutable reference to the state.
    #[inline]
    pub fn state_mut(&mut self) -> &mut S {
        &mut self.state
    }
}

impl<H, V, S> fmt::Debug for HandlerService<H, V, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandlerService").finish_non_exhaustive()
    }
}

impl<C, H, V, S> Service<Cx<C>> for HandlerService<H, V, S>
where
    H: Handler<C, V, S>,
    S: Clone,
{
    type Error = Infallible;
    type Future = HandlerFuture<H::Future>;
    type Response = FlowControl;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all, level = "trace"))]
    fn call(&mut self, cx: Cx<C>) -> Self::Future {
        #[cfg(feature = "tracing")]
        tracing::trace!("calling handler");

        let handler = self.handler.clone();
        let future = handler.call(cx, self.state.clone());
        HandlerFuture::new(future)
    }
}

impl<H, V, S> Clone for HandlerService<H, V, S>
where
    H: Clone,
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            marker: PhantomData,
            handler: self.handler.clone(),
            state: self.state.clone(),
        }
    }
}

pin_project! {
    /// Opaque [`Future`] return type for [`Handler::call`].
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct HandlerFuture<F>
    where
        F: Future<Output = FlowControl>
    {
        #[pin]
        future: HandlerFut<F>,
    }
}

/// Underlying [`HandlerFuture`] type.
type HandlerFut<F> = Map<F, fn(FlowControl) -> Result<FlowControl, Infallible>>;

impl<F> HandlerFuture<F>
where
    F: Future<Output = FlowControl>,
{
    /// Creates a new [`HandlerFuture`].
    pub fn new(future: F) -> Self {
        let future = future.map(Ok as _);
        Self { future }
    }
}

impl<F> Future for HandlerFuture<F>
where
    F: Future<Output = FlowControl>,
{
    type Output = Result<FlowControl, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        this.future.poll(cx)
    }
}

#[cfg(test)]
mod test {
    use crate::Client;
    use crate::backend::Worker;
    use crate::handler::HandlerService;

    fn service<B: Send + 'static>() -> impl Worker<B> {
        async fn handler() {}

        HandlerService::new::<B>(handler, ())
    }

    #[test]
    fn with_debug() {
        // let _ = Daemon::new(backend, service());
    }

    #[test]
    #[cfg(feature = "reqwest")]
    fn with_reqwest() {
        let backend = spire_reqwest::HttpClient::default();
        let _ = Client::new(backend, service());
    }

    #[test]
    #[cfg(feature = "thirtyfour")]
    fn with_thirtyfour() {
        let backend = spire_thirtyfour::BrowserPool::default();
        let _ = Client::new(backend, service());
    }
}
