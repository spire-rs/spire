//! Routing between [`Handler`]s and `tower::`[`Service`]s.
//!

use std::convert::Infallible;
use std::fmt;
use std::sync::Arc;
use std::task::{Context, Poll};

use endpoint::Endpoint;
pub use future::RouteFuture;
use make_route::MakeRoute;
pub use route::Route;
use tag_router::TagRouter;
use tower::{Layer, Service};

use crate::context::{Context as Cx, IntoSignal, Signal, Tag};
pub use crate::handler::{Handler, HandlerService};

mod endpoint;
mod future;
mod make_route;
mod route;
mod tag_router;

/// Composes and routes [`Handler`]s and `tower::`[`Service`]s.
///
/// The router uses `Arc` internally to make cloning cheap.
#[must_use = "services do nothing unless you `.poll_ready` or `.call` them"]
pub struct Router<C = (), S = ()> {
    inner: Arc<TagRouter<C, S>>,
}

impl<C, S> Router<C, S> {
    /// Creates a new [`Router`] of the specified [`Client`] type.
    ///
    /// [`Client`]: crate::backend::Client
    pub fn new() -> Self
    where
        C: 'static,
    {
        let inner = Arc::new(TagRouter::<C, S>::new());
        Self { inner }
    }

    /// Inserts a routed endpoint.
    ///
    /// # Panics
    ///
    /// Panics if a route for this tag has already been inserted.
    pub fn route<H, V>(mut self, tag: impl Into<Tag>, handler: H) -> Self
    where
        C: 'static,
        S: Send + Clone + 'static,
        H: Handler<C, V, S>,
        H::Future: Send + 'static,
        V: Send + 'static,
    {
        let endpoint = Endpoint::from_handler(handler);
        Arc::make_mut(&mut self.inner).route(tag.into(), endpoint);
        self
    }

    /// Inserts a routed endpoint with a provided `tower::`[`Service`].
    ///
    /// # Panics
    ///
    /// Panics if a route for this tag has already been inserted.
    pub fn route_service<H>(mut self, tag: impl Into<Tag>, service: H) -> Self
    where
        C: 'static,
        S: Send + Clone + 'static,
        H: Service<Cx<C>, Error = Infallible> + Clone + Send + 'static,
        H::Response: IntoSignal + 'static,
        H::Future: Send + 'static,
    {
        let endpoint = Endpoint::from_service(service);
        Arc::make_mut(&mut self.inner).route(tag.into(), endpoint);
        self
    }

    /// Replaces the default fallback [`Handler`].
    ///
    /// Fallback handler processes all tasks without matching [`Tag`]s.
    ///
    /// Default handler ignores incoming tasks by returning [`Signal::Continue`].
    ///
    /// # Panics
    ///
    /// Panics if a fallback handler has already been set.
    pub fn fallback<H, V>(mut self, handler: H) -> Self
    where
        C: 'static,
        S: Send + Clone + 'static,
        H: Handler<C, V, S>,
        H::Future: Send + 'static,
        V: Send + 'static,
    {
        let endpoint = Endpoint::from_handler(handler);
        Arc::make_mut(&mut self.inner).fallback(endpoint);
        self
    }

    /// Replaces the default fallback [`Handler`] with a provided `tower::`[`Service`].
    ///
    /// Fallback handler processes all tasks without matching [`Tag`]s.
    ///
    /// Default handler ignores incoming tasks by returning [`Signal::Continue`].
    ///
    /// # Panics
    ///
    /// Panics if a fallback handler has already been set.
    pub fn fallback_service<H>(mut self, service: H) -> Self
    where
        C: 'static,
        S: Send + Clone + 'static,
        H: Service<Cx<C>, Error = Infallible> + Clone + Send + 'static,
        H::Response: IntoSignal + 'static,
        H::Future: Send + 'static,
    {
        let endpoint = Endpoint::from_service(service);
        Arc::make_mut(&mut self.inner).fallback(endpoint);
        self
    }

    /// Merges with another [`Router`] by appending all [`Handler`]s to matching [`Tag`]s.
    pub fn merge(mut self, other: Self) -> Self {
        let other_inner = Arc::try_unwrap(other.inner).unwrap_or_else(|arc| (*arc).clone());
        Arc::make_mut(&mut self.inner).merge(other_inner);
        self
    }

    /// Applies a `tower::`[`Layer`] to all routes in the router.
    ///
    /// This allows you to add middleware to all handlers, such as logging,
    /// rate limiting, or request transformation.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use tower::ServiceBuilder;
    ///
    /// let router = Router::new()
    ///     .route(tag, handler)
    ///     .layer(ServiceBuilder::new().layer(my_middleware));
    /// ```
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
        let inner = Arc::try_unwrap(self.inner).unwrap_or_else(|arc| (*arc).clone());
        let remap = |k: Tag, v: Endpoint<C, S>| (k, v.layer(layer.clone()));
        Self {
            inner: Arc::new(inner.map(remap)),
        }
    }

    /// Attaches state to all handlers in the router.
    ///
    /// This converts a stateless router into a stateful one by providing
    /// state that can be extracted in handlers via the [`State`] extractor.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// #[derive(Clone)]
    /// struct AppState {
    ///     db: Database,
    /// }
    ///
    /// async fn handler(State(state): State<AppState>) {
    ///     // Use state.db
    /// }
    ///
    /// let router = Router::new()
    ///     .route(tag, handler)
    ///     .with_state(AppState { db });
    /// ```
    ///
    /// [`State`]: crate::extract::State
    pub fn with_state<S2>(self, state: S) -> Router<C, S2>
    where
        S: Clone,
    {
        let inner = Arc::try_unwrap(self.inner).unwrap_or_else(|arc| (*arc).clone());
        Router {
            inner: Arc::new(inner.with_state(state)),
        }
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
    type Error = Infallible;
    type Future = RouteFuture<C, Infallible>;
    type Response = Signal;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all, level = "trace"))]
    fn call(&mut self, cx: Cx<C>) -> Self::Future {
        #[cfg(feature = "tracing")]
        tracing::trace!("router calling inner tag router");

        // Clone the Arc to get access to the inner TagRouter
        let mut inner = (*self.inner).clone();
        inner.call(cx)
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
            .route(Tag::from("route1"), handler)
            .route(Tag::from("route2"), handler);
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
            .route(Tag::from("route1"), handler)
            .route(Tag::from("route2"), handler)
            .with_state(AppState::default());
    }
}
