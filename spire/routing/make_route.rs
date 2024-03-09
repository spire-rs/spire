use std::convert::Infallible;
use std::fmt;
use std::sync::Mutex;

use tower::{Layer, Service};

use spire_core::context::Context as Cx;
use spire_core::context::IntoSignal;

use crate::handler::Handler;
use crate::routing::Route;

/// Provides type-erasure for [`Handler`]s.
pub struct MakeRoute<B, S, E>(Mutex<Box<dyn EraseRoute<B, S, E>>>);

impl<B, S> MakeRoute<B, S, Infallible> {
    /// Creates a [`MakeRoute`] from a [`Handler`].
    pub fn new<H, V>(handler: H) -> Self
    where
        B: 'static,
        S: Clone + Send + 'static,
        H: Handler<B, V, S>,
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

impl<B, S, E> MakeRoute<B, S, E> {
    pub fn layer<L, E2>(self, layer: L) -> MakeRoute<B, S, E2>
    where
        B: 'static,
        S: 'static,
        E: 'static,
        L: Layer<Route<B, E>> + Clone + Send + 'static,
        L::Service: Service<Cx<B>> + Clone + Send + 'static,
        <L::Service as Service<Cx<B>>>::Response: IntoSignal + 'static,
        <L::Service as Service<Cx<B>>>::Error: Into<E2> + 'static,
        <L::Service as Service<Cx<B>>>::Future: Send + 'static,
        E2: 'static,
    {
        let f = move |route: Route<B, E>| route.layer(layer.clone());
        self.map(f)
    }

    pub fn map<F, E2>(self, f: F) -> MakeRoute<B, S, E2>
    where
        B: 'static,
        S: 'static,
        E: 'static,
        F: FnOnce(Route<B, E>) -> Route<B, E2> + Clone + Send + 'static,
        E2: 'static,
    {
        let erased = Box::new(EraseLayer {
            inner: self.0.into_inner().unwrap(),
            layer: Box::new(f),
        });

        MakeRoute(Mutex::new(erased))
    }

    /// Converts into the [`Route`].
    pub fn into_route(self, state: S) -> Route<B, E> {
        self.0.into_inner().unwrap().into_route(state)
    }
}

impl<B, S, E> Clone for MakeRoute<B, S, E> {
    fn clone(&self) -> Self {
        let make = self.0.lock().unwrap();
        Self(Mutex::new(make.clone_box()))
    }
}

impl<B, S, E> fmt::Debug for MakeRoute<B, S, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MakeRoute").finish_non_exhaustive()
    }
}

//

trait EraseRoute<B, S, E>: Send {
    fn clone_box(&self) -> Box<dyn EraseRoute<B, S, E>>;

    fn into_route(self: Box<Self>, state: S) -> Route<B, E>;
}

//

struct EraseHandler<B, S, H> {
    handler: H,
    into_route: fn(H, S) -> Route<B, Infallible>,
}

impl<B, S, H> Clone for EraseHandler<B, S, H>
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

impl<B, S, H> EraseRoute<B, S, Infallible> for EraseHandler<B, S, H>
where
    B: 'static,
    H: Clone + Send + 'static,
    S: 'static,
{
    fn clone_box(&self) -> Box<dyn EraseRoute<B, S, Infallible>> {
        Box::new(self.clone())
    }

    fn into_route(self: Box<Self>, state: S) -> Route<B, Infallible> {
        (self.into_route)(self.handler, state)
    }
}

//

struct EraseLayer<B, S, E, E2> {
    inner: Box<dyn EraseRoute<B, S, E>>,
    layer: Box<dyn LayerFn<B, E, E2>>,
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
