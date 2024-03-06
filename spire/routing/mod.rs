use std::convert::Infallible;
use std::fmt;
use std::task::{Context, Poll};

use tower::{Layer, Service};

use spire_core::backend::Backend;
use spire_core::context::{Context as Cx, Tag};
use spire_core::process::{IntoSignal, Signal};

use crate::handler::Handler;
use crate::routing::endpoint::Endpoint;
pub use crate::routing::future::RouteFuture;
use crate::routing::make_route::MakeRoute;
pub use crate::routing::route::Route;
use crate::routing::tag_router::TagRouter;

mod endpoint;
mod future;
mod make_route;
mod route;
mod tag_router;

pub struct Router<B = (), S = ()> {
    inner: TagRouter<B, S>,
}

impl<B, S> Router<B, S> {
    /// Creates a new [`Router`] with a given [`Backend`].
    pub fn new() -> Self
    where
        B: Backend,
    {
        let inner = TagRouter::<B, S>::new();
        Self { inner }
    }

    pub fn route<H, V>(mut self, tag: impl Into<Tag>, handler: H) -> Self
    where
        B: Backend,
        S: Send + Clone + 'static,
        H: Handler<B, V, S>,
        H::Future: Send + 'static,
        V: Send + 'static,
    {
        let endpoint = Endpoint::from_handler(handler);
        self.inner.route(tag.into(), endpoint);
        self
    }

    pub fn route_service<H>(mut self, tag: impl Into<Tag>, service: H) -> Self
    where
        B: Backend,
        S: Send + Clone + 'static,
        H: Service<Cx<B>, Error = Infallible> + Clone + Send + 'static,
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
    ///
    /// [`Signal::Continue`]: crate::handler::Signal::Continue
    pub fn fallback<H, V>(mut self, handler: H) -> Self
    where
        B: Backend,
        S: Send + Clone + 'static,
        H: Handler<B, V, S>,
        H::Future: Send + 'static,
        V: Send + 'static,
    {
        let endpoint = Endpoint::from_handler(handler);
        self.inner.fallback(endpoint);
        self
    }

    pub fn fallback_service<H>(mut self, service: H) -> Self
    where
        B: Backend,
        S: Send + Clone + 'static,
        H: Service<Cx<B>, Error = Infallible> + Clone + Send + 'static,
        H::Response: IntoSignal + 'static,
        H::Future: Send + 'static,
    {
        let endpoint = Endpoint::from_service(service);
        self.inner.fallback(endpoint);
        self
    }

    /// Merges with another [`Router`] by appending all [`Handler`]s to matching [`Tag`]s.
    pub fn merge(mut self, other: Router<B, S>) -> Self {
        self.inner.merge(other.inner);
        self
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
        Self {
            inner: self.inner.layer(layer),
        }
    }

    pub fn with_state<S2>(self, state: S) -> Router<B, S2> {
        let inner = self.inner.with_state(state);
        Router { inner }
    }
}

impl<B, S> Clone for Router<B, S> {
    fn clone(&self) -> Self {
        let inner = self.inner.clone();
        Self { inner }
    }
}

impl<B, S> fmt::Display for Router<B, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Router").finish_non_exhaustive()
    }
}

impl<B, S> Default for Router<B, S>
where
    B: Backend,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<B> Service<Cx<B>> for Router<B, ()> {
    type Response = Signal;
    type Error = Infallible;
    type Future = RouteFuture<B, Infallible>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, cx: Cx<B>) -> Self::Future {
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

        let router: Router = Router::new()
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

        let router: Router = Router::new()
            .route(Tag::default(), handler)
            .route(Tag::default(), handler)
            .with_state(AppState::default());
    }
}
