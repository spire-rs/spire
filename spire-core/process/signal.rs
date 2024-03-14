use std::convert::Infallible;
use std::task::{Context, Poll};

use tower::{Layer, Service};

use crate::context::{Context as Cx, Signal};

/// TODO.
#[derive(Debug, Clone)]
pub struct Signals<S> {
    inner: S,
}

impl<S> Signals<S> {
    /// Creates a new [`Signals`] service.
    pub fn new(inner: S) -> Self {
        Self { inner }
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

    /// TODO.
    pub async fn notify_signal(&self, signal: Signal) {
        todo!()
    }
}

impl<B, S> Service<Cx<B>> for Signals<S>
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

#[derive(Debug, Default, Clone)]
pub struct SignalsLayer {
    _marker: (),
}

impl<S> Layer<S> for SignalsLayer {
    type Service = Signals<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Signals::new(inner)
    }
}
