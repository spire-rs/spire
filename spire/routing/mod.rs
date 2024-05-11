//! Routing between [`Handler`]s and `tower::`[`Service`]s.
//!

use std::convert::Infallible;
use std::fmt;
use std::task::{Context, Poll};

use tower::{Layer, Service};

use endpoint::Endpoint;
pub use future::RouteFuture;
use make_route::MakeRoute;
pub use route::Route;
use tag_router::TagRouter;

use crate::context::{Context as Cx, IntoSignal, Signal, Tag};
pub use crate::handler::{Handler, HandlerService};

mod endpoint;
mod future;
mod make_route;
mod route;
mod tag_router;

/// Composes and routes [`Handler`]s and `tower::`[`Service`]s.
#[must_use = "services do nothing unless you `.poll_ready` or `.call` them"]
pub struct Router<C = (), S = ()> {
    // TODO: Use Arc<TagRouter<C, S>>.
    inner: TagRouter<C, S>,
}

impl<C, S> Router<C, S> {
    /// Creates a new [`Router`] of the specified [`Backend`] type.
    ///
    /// [`Backend`]: crate::backend::Backend
    pub fn new() -> Self
    where
        C: 'static,
    {
        let inner = TagRouter::<C, S>::new();
        Self { inner }
    }

    pub fn route<H, V>(mut self, tag: impl Into<Tag>, handler: H) -> Self
    where
        C: 'static,
        S: Send + Clone + 'static,
        H: Handler<C, V, S>,
        H::Future: Send + 'static,
        V: Send + 'static,
    {
        let endpoint = Endpoint::from_handler(handler);
        self.inner.route(tag.into(), endpoint);
        self
    }

    pub fn route_service<H>(mut self, tag: impl Into<Tag>, service: H) -> Self
    where
        C: 'static,
        S: Send + Clone + 'static,
        H: Service<Cx<C>, Error = Infallible> + Clone + Send + 'static,
        H::Response: IntoSignal + 'static,
        H::Future: Send + 'static,
    {
        let endpoint = Endpoint::from_service(service);
        self.inner.route(tag.into(), endpoint);
        self
    }

    /// Replaces the current fallback [`Handler`].
    /// Fallback handler processes all tasks without matching [`Tag`]s.
    ///
    /// Default handler ignores incoming tasks by returning [`Signal::Continue`].
    pub fn fallback<H, V>(mut self, handler: H) -> Self
    where
        C: 'static,
        S: Send + Clone + 'static,
        H: Handler<C, V, S>,
        H::Future: Send + 'static,
        V: Send + 'static,
    {
        let endpoint = Endpoint::from_handler(handler);
        self.inner.fallback(endpoint);
        self
    }

    /// Replaces the current fallback [`Handler`] with a provided `tower::`[`Service`].
    /// Fallback handler processes all tasks without matching [`Tag`]s.
    ///
    /// Default handler ignores incoming tasks by returning [`Signal::Continue`].
    pub fn fallback_service<H>(mut self, service: H) -> Self
    where
        C: 'static,
        S: Send + Clone + 'static,
        H: Service<Cx<C>, Error = Infallible> + Clone + Send + 'static,
        H::Response: IntoSignal + 'static,
        H::Future: Send + 'static,
    {
        let endpoint = Endpoint::from_service(service);
        self.inner.fallback(endpoint);
        self
    }

    /// Merges with another [`Router`] by appending all [`Handler`]s to matching [`Tag`]s.
    pub fn merge(mut self, other: Self) -> Self {
        self.inner.merge(other.inner);
        self
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
        let remap = |k: Tag, v: Endpoint<C, S>| (k, v.layer(layer.clone()));
        Self {
            inner: self.inner.layer(remap),
        }
    }

    pub fn with_state<S2>(self, state: S) -> Router<C, S2>
    where
        S: Clone,
    {
        let inner = self.inner.with_state(state);
        Router { inner }
    }
}

impl<C, S> Clone for Router<C, S> {
    fn clone(&self) -> Self {
        let inner = self.inner.clone();
        Self { inner }
    }
}

impl<C, S> fmt::Display for Router<C, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Router").finish_non_exhaustive()
    }
}

impl<C, S> Default for Router<C, S>
where
    C: 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<C> Service<Cx<C>> for Router<C, ()>
where
    C: 'static,
{
    type Response = Signal;
    type Error = Infallible;
    type Future = RouteFuture<C, Infallible>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, cx: Cx<C>) -> Self::Future {
        self.inner.call(cx)
    }
}

#[cfg(test)]
mod test {
    use crate::context::Tag;
    use crate::extract::{FromRef, State};
    use crate::routing::Router;

    #[test]
    fn basic_routing() {
        async fn handler() {}

        let _: Router = Router::new()
            .route(Tag::default(), handler)
            .route(Tag::default(), handler);
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

        let _: Router = Router::new()
            .route(Tag::default(), handler)
            .route(Tag::default(), handler)
            .with_state(AppState::default());
    }
}
