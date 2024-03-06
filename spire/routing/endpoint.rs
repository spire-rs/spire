use std::convert::Infallible;
use std::fmt;
use std::task::{Context, Poll};

use tower::{Layer, Service};

use spire_core::backend::Backend;
use spire_core::context::Context as Cx;
use spire_core::context::{IntoSignal, Signal};

use crate::handler::Handler;
use crate::routing::{MakeRoute, Route, RouteFuture};

/// TODO.
pub enum Endpoint<B, S> {
    /// TODO. [`Service`]
    Route(Route<B, Infallible>),
    /// TODO. [`Handler`]
    Handler(MakeRoute<B, S, Infallible>),
}

impl<B, S> Endpoint<B, S> {
    pub fn from_service<T>(service: T) -> Self
    where
        B: Backend,
        T: Service<Cx<B>, Error = Infallible> + Clone + Send + 'static,
        T::Response: IntoSignal + 'static,
        T::Future: Send + 'static,
    {
        Endpoint::Route(Route::new(service))
    }

    pub fn from_handler<H, V>(handler: H) -> Self
    where
        B: Backend,
        S: Clone + Send + 'static,
        H: Handler<B, V, S>,
        H::Future: Send + 'static,
        V: Send + 'static,
    {
        Endpoint::Handler(MakeRoute::new(handler))
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
            Endpoint::Route(x) => Endpoint::Route(x.layer(layer)),
            Endpoint::Handler(x) => Endpoint::Handler(x.layer(layer)),
        }
    }

    pub fn with_state<S2>(self, state: S) -> Endpoint<B, S2> {
        match self {
            Endpoint::Route(x) => Endpoint::Route(x),
            Endpoint::Handler(x) => Endpoint::Route(x.into_route(state)),
        }
    }
}

impl<B, S> Clone for Endpoint<B, S> {
    fn clone(&self) -> Self {
        match self {
            Endpoint::Route(x) => Endpoint::Route(x.clone()),
            Endpoint::Handler(x) => Endpoint::Handler(x.clone()),
        }
    }
}

impl<B, S> fmt::Debug for Endpoint<B, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Endpoint::Route(x) => x.fmt(f),
            Endpoint::Handler(x) => x.fmt(f),
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
            Endpoint::Route(x) => x.call(cx),
            Endpoint::Handler(x) => x.clone().into_route(()).call(cx),
        }
    }
}
