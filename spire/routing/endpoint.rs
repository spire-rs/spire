use std::convert::Infallible;
use std::fmt;
use std::task::{Context, Poll};

use tower::{Layer, Service};

use crate::context::{Context as Cx, IntoSignal, Signal};
use crate::handler::Handler;
use crate::routing::{MakeRoute, Route, RouteFuture};

pub enum Endpoint<C, S> {
    /// Cloneable [`Service`].
    Route(Route<C, Infallible>),
    /// [`Handler`] without state.
    Handler(MakeRoute<C, S, Infallible>),
}

impl<C, S> Endpoint<C, S> {
    pub fn from_service<T>(service: T) -> Self
    where
        T: Service<Cx<C>, Error = Infallible> + Clone + Send + 'static,
        T::Response: IntoSignal + 'static,
        T::Future: Send + 'static,
    {
        Self::Route(Route::new(service))
    }

    pub fn from_handler<H, V>(handler: H) -> Self
    where
        C: 'static,
        S: Clone + Send + 'static,
        H: Handler<C, V, S>,
        H::Future: Send + 'static,
        V: Send + 'static,
    {
        Self::Handler(MakeRoute::new(handler))
    }

    pub fn layer<L>(self, layer: L) -> Self
    where
        C: 'static,
        S: Clone + Send + 'static,
        L: Layer<Route<C, Infallible>> + Clone + Send + 'static,
        L::Service: Service<Cx<C>> + Clone + Send + 'static,
        <L::Service as Service<Cx<C>>>::Response: IntoSignal + 'static,
        <L::Service as Service<Cx<C>>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Cx<C>>>::Future: Send + 'static,
    {
        match self {
            Self::Route(x) => Self::Route(x.layer(layer)),
            Self::Handler(x) => Self::Handler(x.layer(layer)),
        }
    }

    pub fn with_state<S2>(self, state: S) -> Endpoint<C, S2> {
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

impl<C> Default for Endpoint<C, ()>
where
    C: 'static,
{
    #[inline]
    fn default() -> Self {
        Self::from_handler(default_fallback)
    }
}

impl<C, S> Clone for Endpoint<C, S> {
    fn clone(&self) -> Self {
        match self {
            Self::Route(x) => Self::Route(x.clone()),
            Self::Handler(x) => Self::Handler(x.clone()),
        }
    }
}

impl<C, S> fmt::Debug for Endpoint<C, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Route(x) => x.fmt(f),
            Self::Handler(x) => x.fmt(f),
        }
    }
}

// TODO: fn invoke(&self, Cx<C>) -> Signal

impl<C> Service<Cx<C>> for Endpoint<C, ()> {
    type Response = Signal;
    type Error = Infallible;
    type Future = RouteFuture<C, Infallible>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, cx: Cx<C>) -> Self::Future {
        match self {
            Self::Route(x) => x.call(cx),
            Self::Handler(x) => x.clone().into_route(()).call(cx),
        }
    }
}
