use std::convert::Infallible;
use std::fmt;
use std::marker::PhantomData;
use std::task::{Context as TaskContext, Poll};

use futures::FutureExt;
use futures::future::BoxFuture;
use tower::Service;

use crate::Context;
use crate::handler::Handler;
use crate::worker::Outcome;

/// A [`Handler`] bound to its state, as a tower [`Service`].
///
/// Created by [`Handler::with_state`]; routers create them for every route.
#[must_use = "services do nothing unless you `.poll_ready` or `.call` them"]
pub struct HandlerService<H, M, S> {
    handler: H,
    state: S,
    marker: PhantomData<fn() -> M>,
}

impl<H, M, S> HandlerService<H, M, S> {
    /// Binds `handler` to `state`.
    pub const fn new(handler: H, state: S) -> Self {
        Self {
            handler,
            state,
            marker: PhantomData,
        }
    }

    /// Returns the state.
    #[must_use]
    pub const fn state(&self) -> &S {
        &self.state
    }
}

impl<H: Clone, M, S: Clone> Clone for HandlerService<H, M, S> {
    fn clone(&self) -> Self {
        Self::new(self.handler.clone(), self.state.clone())
    }
}

impl<H, M, S> fmt::Debug for HandlerService<H, M, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandlerService").finish_non_exhaustive()
    }
}

impl<C, H, M, S> Service<Context<C>> for HandlerService<H, M, S>
where
    H: Handler<C, M, S>,
    S: Clone,
{
    type Error = Infallible;
    type Future = BoxFuture<'static, Result<Outcome, Infallible>>;
    type Response = Outcome;

    fn poll_ready(&mut self, _cx: &mut TaskContext<'_>) -> Poll<Result<(), Infallible>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, cx: Context<C>) -> Self::Future {
        let future = self.handler.clone().call(cx, self.state.clone());
        future.map(Ok).boxed()
    }
}
