//! Type-erased handler construction.
//!
//! This module provides [`MakeRoute`], which enables type erasure for handlers
//! that haven't been attached to state yet. This allows routers to store handlers
//! before state is available, deferring the conversion to [`Route`] until later.

use std::convert::Infallible;
use std::fmt;
use std::sync::Mutex;

use tower::{Layer, Service};

use crate::context::{Context as Cx, IntoFlowControl};
use crate::handler::Handler;
use crate::routing::Route;

/// Provides type-erasure for [`Handler`]s.
pub struct MakeRoute<C, S, E>(Mutex<Box<dyn EraseRoute<C, S, E>>>);

impl<C, S> MakeRoute<C, S, Infallible> {
    /// Creates a [`MakeRoute`] from a [`Handler`].
    pub fn new<H, V>(handler: H) -> Self
    where
        C: 'static,
        S: Clone + Send + 'static,
        H: Handler<C, V, S>,
        H::Future: Send + 'static,
        V: Send + 'static,
    {
        let into_route = |handler, state| {
            let svc = Handler::with_state(handler, state);
            Route::new(svc)
        };

        let erased = Box::new(EraseHandler {
            handler,
            into_route,
        });

        Self(Mutex::new(erased))
    }
}

impl<C, S, E> MakeRoute<C, S, E> {
    pub fn layer<L, E2>(self, layer: L) -> MakeRoute<C, S, E2>
    where
        C: 'static,
        S: 'static,
        E: 'static,
        L: Layer<Route<C, E>> + Clone + Send + 'static,
        L::Service: Service<Cx<C>> + Clone + Send + 'static,
        <L::Service as Service<Cx<C>>>::Response: IntoFlowControl + 'static,
        <L::Service as Service<Cx<C>>>::Error: Into<E2> + 'static,
        <L::Service as Service<Cx<C>>>::Future: Send + 'static,
        E2: 'static,
    {
        let f = move |route: Route<C, E>| route.layer(layer.clone());
        self.map(f)
    }

    pub fn map<F, E2>(self, f: F) -> MakeRoute<C, S, E2>
    where
        C: 'static,
        S: 'static,
        E: 'static,
        F: FnOnce(Route<C, E>) -> Route<C, E2> + Clone + Send + 'static,
        E2: 'static,
    {
        let erased = Box::new(EraseLayer {
            inner: self.0.into_inner().unwrap(),
            layer: Box::new(f),
        });

        MakeRoute(Mutex::new(erased))
    }

    /// Converts into the [`Route`].
    pub fn into_route(self, state: S) -> Route<C, E> {
        self.0.into_inner().unwrap().into_route(state)
    }
}

impl<C, S, E> Clone for MakeRoute<C, S, E> {
    fn clone(&self) -> Self {
        let make = self.0.lock().unwrap();
        Self(Mutex::new(make.clone_box()))
    }
}

impl<C, S, E> fmt::Debug for MakeRoute<C, S, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MakeRoute").finish_non_exhaustive()
    }
}

//

trait EraseRoute<C, S, E>: Send {
    fn clone_box(&self) -> Box<dyn EraseRoute<C, S, E>>;

    fn into_route(self: Box<Self>, state: S) -> Route<C, E>;
}

//

struct EraseHandler<C, S, H> {
    handler: H,
    into_route: fn(H, S) -> Route<C, Infallible>,
}

impl<C, S, H> Clone for EraseHandler<C, S, H>
where
    H: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            into_route: self.into_route,
        }
    }
}

impl<C, S, H> EraseRoute<C, S, Infallible> for EraseHandler<C, S, H>
where
    C: 'static,
    H: Clone + Send + 'static,
    S: 'static,
{
    fn clone_box(&self) -> Box<dyn EraseRoute<C, S, Infallible>> {
        Box::new(self.clone())
    }

    fn into_route(self: Box<Self>, state: S) -> Route<C, Infallible> {
        (self.into_route)(self.handler, state)
    }
}

//

struct EraseLayer<C, S, E, E2> {
    inner: Box<dyn EraseRoute<C, S, E>>,
    layer: Box<dyn LayerFn<C, E, E2>>,
}

impl<B, S, E, E2> EraseRoute<B, S, E2> for EraseLayer<B, S, E, E2>
where
    B: 'static,
    S: 'static,
    E: 'static,
    E2: 'static,
{
    fn clone_box(&self) -> Box<dyn EraseRoute<B, S, E2>> {
        Box::new(Self {
            inner: self.inner.clone_box(),
            layer: self.layer.clone_box(),
        })
    }

    fn into_route(self: Box<Self>, state: S) -> Route<B, E2> {
        (self.layer)(self.inner.into_route(state))
    }
}

trait LayerFn<B, E, E2>: FnOnce(Route<B, E>) -> Route<B, E2> + Send {
    fn clone_box(&self) -> Box<dyn LayerFn<B, E, E2>>;
}

impl<B, E, E2, F> LayerFn<B, E, E2> for F
where
    F: FnOnce(Route<B, E>) -> Route<B, E2> + Clone + Send + 'static,
{
    fn clone_box(&self) -> Box<dyn LayerFn<B, E, E2>> {
        Box::new(self.clone())
    }
}
