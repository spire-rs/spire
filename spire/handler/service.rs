use std::convert::Infallible;
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::{future::Map, FutureExt};
use pin_project_lite::pin_project;
use tower::Service;

use spire_core::context::Context as Cx;

use crate::handler::{Handler, Signal};

pub struct HandlerService<H, V, S> {
    marker: PhantomData<V>,
    handler: H,
    state: S,
}

impl<H, V, S> HandlerService<H, V, S> {
    /// Creates a new [`HandlerService`].
    pub fn new(handler: H, state: S) -> Self {
        Self {
            marker: PhantomData,
            handler,
            state,
        }
    }

    /// Gets a reference to the state.
    pub fn state_ref(&self) -> &S {
        &self.state
    }

    /// Gets a mutable reference to the state.
    pub fn state_mut(&mut self) -> &mut S {
        &mut self.state
    }
}

impl<H, V, S> fmt::Debug for HandlerService<H, V, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandlerService").finish_non_exhaustive()
    }
}

impl<B, H, V, S> Service<Cx<B>> for HandlerService<H, V, S>
where
    H: Handler<B, V, S>,
    S: Clone,
{
    type Response = Signal;
    type Error = Infallible;
    type Future = HandlerFuture<H::Future>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, cx: Cx<B>) -> Self::Future {
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
    pub struct HandlerFuture<F>
    where
        F: Future<Output = Signal>
    {
        #[pin]
        future: HandlerFut<F>,
    }
}

/// Underlying [`HandlerFuture`] type.
type HandlerFut<F> = Map<F, fn(Signal) -> Result<Signal, Infallible>>;

impl<F> HandlerFuture<F>
where
    F: Future<Output = Signal>,
{
    /// Creates a new [`HandlerFuture`].
    pub fn new(future: F) -> Self {
        let future = future.map(Ok as _);
        Self { future }
    }
}

impl<F> Future for HandlerFuture<F>
where
    F: Future<Output = Signal>,
{
    type Output = Result<Signal, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        this.future.poll(cx)
    }
}
