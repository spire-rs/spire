use std::convert::Infallible;
use std::fmt;
use std::task::{Context, Poll};

use tower::{Layer, Service};

use spire_core::context::Context as Cx;
use spire_core::context::{IntoSignal, Signal};

use crate::handler::Handler;
use crate::routing::{MakeRoute, Route, RouteFuture};

pub enum Endpoint<B, S> {
    /// Cloneable [`Service`].
    Route(Route<B, Infallible>),
    /// [`Handler`] without state.
    Handler(MakeRoute<B, S, Infallible>),
}

impl<B, S> Endpoint<B, S> {
    pub fn from_service<T>(service: T) -> Self
    where
        T: Service<Cx<B>, Error = Infallible> + Clone + Send + 'static,
        T::Response: IntoSignal + 'static,
        T::Future: Send + 'static,
    {
        Self::Route(Route::new(service))
    }

    pub fn from_handler<H, V>(handler: H) -> Self
    where
        B: 'static,
        S: Clone + Send + 'static,
        H: Handler<B, V, S>,
        H::Future: Send + 'static,
        V: Send + 'static,
    {
        Self::Handler(MakeRoute::new(handler))
    }

    pub fn layer<L>(self, layer: L) -> Self
    where
        B: 'static,
        S: Clone + Send + 'static,
        L: Layer<Route<B, Infallible>> + Clone + Send + 'static,
        L::Service: Service<Cx<B>> + Clone + Send + 'static,
        <L::Service as Service<Cx<B>>>::Response: IntoSignal + 'static,
        <L::Service as Service<Cx<B>>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Cx<B>>>::Future: Send + 'static,
    {
        match self {
            Self::Route(x) => Self::Route(x.layer(layer)),
            Self::Handler(x) => Self::Handler(x.layer(layer)),
        }
    }

    pub fn with_state<S2>(self, state: S) -> Endpoint<B, S2> {
        match self {
            Self::Route(x) => Endpoint::Route(x),
            Self::Handler(x) => Endpoint::Route(x.into_route(state)),
        }
    }
}

/// Ignores all incoming tasks by returning [`Signal::Continue`].
async fn default_fallback() -> Signal {
    Signal::Continue
}

impl<B> Default for Endpoint<B, ()>
where
    B: 'static,
{
    fn default() -> Self {
        Self::from_handler(default_fallback)
    }
}

impl<B, S> Clone for Endpoint<B, S> {
    fn clone(&self) -> Self {
        match self {
            Self::Route(x) => Self::Route(x.clone()),
            Self::Handler(x) => Self::Handler(x.clone()),
        }
    }
}

impl<B, S> fmt::Debug for Endpoint<B, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Route(x) => x.fmt(f),
            Self::Handler(x) => x.fmt(f),
        }
    }
}

impl<B> Service<Cx<B>> for Endpoint<B, ()> {
    type Response = Signal;
    type Error = Infallible;
    type Future = RouteFuture<B, Infallible>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, cx: Cx<B>) -> Self::Future {
        match self {
            Self::Route(x) => x.call(cx),
            Self::Handler(x) => x.clone().into_route(()).call(cx),
        }
    }
}
