use std::convert::Infallible;
use std::fmt;
use std::marker::PhantomData;
use std::task::{Context, Poll};

use futures_util::future::Map;
use tower::Service;

use spire_core::Signal;

use crate::handler::{Handler, HandlerContext};

/// TODO: Make opaque.
pub type HandlerFuture<F> = Map<F, fn(Signal) -> Result<Signal, Infallible>>;

/// An adapter that makes a [`Handler`] into a [`Service`].
pub struct HandlerService<H, T, S> {
    marker: PhantomData<T>,
    handler: H,
    state: S,
}

impl<H, T, S> HandlerService<H, T, S> {
    pub fn new(handler: H, state: S) -> Self {
        Self {
            marker: PhantomData,
            handler,
            state,
        }
    }

    /// Gets a reference to the state.
    pub fn state(&self) -> &S {
        &self.state
    }

    /// Gets a mutable reference to the state.
    pub fn state_mut(&mut self) -> &mut S {
        &mut self.state
    }
}

impl<H, T, S> fmt::Debug for HandlerService<H, T, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandlerService").finish_non_exhaustive()
    }
}

impl<H, T, S> Clone for HandlerService<H, T, S>
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

impl<H, T, S> Service<HandlerContext> for HandlerService<H, T, S>
where
    H: Handler<T, S> + Clone + Send + 'static,
    S: Clone + Send + Sync,
{
    type Response = Signal;
    type Error = Infallible;
    type Future = HandlerFuture<<H as Handler<T, S>>::Future>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, cx: HandlerContext) -> Self::Future {
        use futures_util::future::FutureExt;

        let handler = self.handler.clone();
        let future = handler.call(cx, self.state.clone());
        future.map(Ok as _)
    }
}

// pub struct BoxHandlerService {}
