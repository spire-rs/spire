//! Type-erased routing service.
//!
//! This module provides the [`Route`] type, which wraps a `tower::Service`
//! with type erasure to allow heterogeneous collections of routes.

use std::fmt;
use std::sync::Mutex;
use std::task::{Context, Poll};

use tower::util::{BoxCloneService, Oneshot};
use tower::util::{MapErrLayer, MapResponseLayer};
use tower::{Layer, Service, ServiceExt};

use crate::context::{Context as Cx, IntoSignal, Signal};
use crate::routing::RouteFuture;

/// Provides type-erasure for the underlying `tower::`[`Service`].
pub struct Route<C, E> {
    inner: Mutex<BoxCloneService<Cx<C>, Signal, E>>,
}

impl<C, E> Route<C, E> {
    /// Creates a new [`Route`].
    pub fn new<T>(svc: T) -> Self
    where
        T: Service<Cx<C>, Error = E> + Clone + Send + 'static,
        T::Response: IntoSignal + 'static,
        T::Future: Send + 'static,
    {
        let inner = Mutex::new(BoxCloneService::new(
            svc.map_response(IntoSignal::into_signal),
        ));

        Self { inner }
    }

    /// Calls the underlying `tower::`[`Service`] with a provided [`Context`].
    ///
    /// [`Context`]: Cx
    fn oneshot_inner(&mut self, cx: Cx<C>) -> Oneshot<BoxCloneService<Cx<C>, Signal, E>, Cx<C>> {
        let svc = self.inner.lock().unwrap();
        svc.clone().oneshot(cx)
    }

    /// Applies a `tower::`[`Layer`] to the [`Route`].
    pub fn layer<L, E2>(self, layer: L) -> Route<C, E2>
    where
        L: Layer<Self> + Clone + Send + 'static,
        L::Service: Service<Cx<C>> + Clone + Send + 'static,
        <L::Service as Service<Cx<C>>>::Response: IntoSignal + 'static,
        <L::Service as Service<Cx<C>>>::Error: Into<E2> + 'static,
        <L::Service as Service<Cx<C>>>::Future: Send + 'static,
        E2: 'static,
    {
        let layer = (
            MapErrLayer::new(Into::into),
            MapResponseLayer::new(IntoSignal::into_signal),
            layer,
        );

        Route::new(layer.layer(self))
    }
}

impl<C, E> Clone for Route<C, E> {
    fn clone(&self) -> Self {
        let svc = self.inner.lock().unwrap();

        Self {
            inner: Mutex::new(svc.clone()),
        }
    }
}

impl<C, E> fmt::Debug for Route<C, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Route").finish_non_exhaustive()
    }
}

impl<C, E> Service<Cx<C>> for Route<C, E> {
    type Response = Signal;
    type Error = E;
    type Future = RouteFuture<C, E>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, cx: Cx<C>) -> Self::Future {
        RouteFuture::new(self.oneshot_inner(cx))
    }
}

#[cfg(test)]
mod test {
    use crate::extract::{FromRef, State};
    use crate::handler::Handler;

    #[test]
    fn basic_routing() {
        async fn handler() {}

        let svc = Handler::<(), _, _>::with_state(handler, ());
        // let cx = Context::new(());
        // let _ = Route::new(svc).call(cx);
    }

    #[test]
    fn state_routing() {
        #[derive(Debug, Default, Clone)]
        struct AppState {
            sub: u32,
        }

        impl FromRef<AppState> for u32 {
            fn from_ref(input: &AppState) -> Self {
                input.sub.clone()
            }
        }

        async fn handler(_: State<AppState>, _: State<u32>) {}

        let svc = Handler::<(), _, _>::with_state(handler, AppState::default());
        // let cx = Context::new(());
        // let _ = Route::new(svc).call(cx);
    }
}
