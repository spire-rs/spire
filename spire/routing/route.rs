use std::fmt;
use std::sync::Mutex;
use std::task::{Context, Poll};

use tower::util::{BoxCloneService, Oneshot};
use tower::util::{MapErrLayer, MapResponseLayer};
use tower::{Layer, Service, ServiceExt};

use spire_core::context::Context as Cx;
use spire_core::context::{IntoSignal, Signal};

use crate::routing::RouteFuture;

/// Provides type-erasure for the underlying `tower::`[`Service`].
pub struct Route<B, E> {
    inner: Mutex<BoxCloneService<Cx<B>, Signal, E>>,
}

impl<B, E> Route<B, E> {
    /// Creates a new [`Route`].
    pub fn new<T>(svc: T) -> Self
    where
        T: Service<Cx<B>, Error = E> + Clone + Send + 'static,
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
    fn oneshot_inner(&mut self, cx: Cx<B>) -> Oneshot<BoxCloneService<Cx<B>, Signal, E>, Cx<B>> {
        let svc = self.inner.lock().unwrap();
        svc.clone().oneshot(cx)
    }

    /// Applies a `tower::`[`Layer`] to the [`Route`].
    pub fn layer<L, E2>(self, layer: L) -> Route<B, E2>
    where
        L: Layer<Route<B, E>> + Clone + Send + 'static,
        L::Service: Service<Cx<B>> + Clone + Send + 'static,
        <L::Service as Service<Cx<B>>>::Response: IntoSignal + 'static,
        <L::Service as Service<Cx<B>>>::Error: Into<E2> + 'static,
        <L::Service as Service<Cx<B>>>::Future: Send + 'static,
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

impl<B, E> Clone for Route<B, E> {
    fn clone(&self) -> Self {
        let svc = self.inner.lock().unwrap();
        let inner = Mutex::new(svc.clone());
        Self { inner }
    }
}

impl<B, E> fmt::Debug for Route<B, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Route").finish_non_exhaustive()
    }
}

impl<B, E> Service<Cx<B>> for Route<B, E> {
    type Response = Signal;
    type Error = E;
    type Future = RouteFuture<B, E>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, cx: Cx<B>) -> Self::Future {
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
