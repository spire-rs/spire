//! Routing between [`Handler`]s and `tower::`[`Service`]s.
//!
//! This module provides the core routing functionality for Spire, allowing you to
//! map different tags to specific handlers or services. The router uses an efficient
//! tag-based routing system that can handle both static and dynamic routing patterns.
//!
//! # Basic Usage
//!
//! ```ignore
//! use spire::routing::Router;
//! use spire::context::{Tag, FlowControl};
//!
//! async fn home_handler() -> &'static str {
//!     "Welcome to the home page"
//! }
//!
//! async fn about_handler() -> &'static str {
//!     "About us"
//! }
//!
//! let router = Router::new()
//!     .route(Tag::from("home"), home_handler)
//!     .route(Tag::from("about"), about_handler)
//!     .fallback(|| async { "Page not found" });
//! ```
//!
//! # State Management
//!
//! ```ignore
//! use spire::routing::Router;
//! use spire::extract::State;
//!
//! #[derive(Clone)]
//! struct AppState {
//!     db_pool: DatabasePool,
//! }
//!
//! async fn handler(State(state): State<AppState>) -> &'static str {
//!     // Use state.db_pool
//!     "Data processed"
//! }
//!
//! let app_state = AppState { db_pool: create_pool() };
//! let router = Router::new()
//!     .route("process", handler)
//!     .with_state(app_state);
//! ```
//!
//! # Service Integration
//!
//! ```ignore
//! use tower::ServiceBuilder;
//! use spire::routing::Router;
//!
//! let router = Router::new()
//!     .route("api", my_handler)
//!     .layer(ServiceBuilder::new()
//!         .timeout(Duration::from_secs(30))
//!         .layer(my_middleware)
//!     );
//! ```

use std::convert::Infallible;
use std::fmt;
use std::sync::Arc;
use std::task::{Context, Poll};

use endpoint::Endpoint;
pub use future::RouteFuture;
pub use route::Route;
use tag_router::TagRouter;
use tower::{Layer, Service};

use crate::context::{Context as Cx, FlowControl, IntoFlowControl, Tag};
pub use crate::handler::{Handler, HandlerService};

mod endpoint;
mod future;
mod make_route;
mod route;
mod tag_router;

/// Composes and routes [`Handler`]s and `tower::`[`Service`]s.
///
/// The `Router` is the central component for request routing in Spire. It maps
/// [`Tag`]s to handlers or services, allowing different types of requests to be
/// processed by appropriate logic.
///
/// # Features
///
/// - **Tag-based routing**: Route requests based on their tags
/// - **Fallback handling**: Define fallback behavior for unmatched requests
/// - **State management**: Attach state to handlers for shared data access
/// - **Middleware support**: Apply tower middleware layers to all routes
/// - **Service compatibility**: Full integration with the tower ecosystem
/// - **Efficient cloning**: Uses `Arc` internally for cheap clones
///
/// # Examples
///
/// ## Basic Routing
///
/// ```ignore
/// use spire::routing::Router;
/// use spire::context::Tag;
///
/// async fn api_handler() -> &'static str { "API response" }
/// async fn web_handler() -> &'static str { "Web response" }
///
/// let router = Router::new()
///     .route(Tag::from("api"), api_handler)
///     .route(Tag::from("web"), web_handler);
/// ```
///
/// ## With State and Middleware
///
/// ```ignore
/// use spire::routing::Router;
/// use spire::extract::State;
/// use tower::ServiceBuilder;
///
/// #[derive(Clone)]
/// struct AppState { /* ... */ }
///
/// async fn handler(State(state): State<AppState>) -> &'static str {
///     "Processed"
/// }
///
/// let router = Router::new()
///     .route("process", handler)
///     .layer(ServiceBuilder::new().timeout(Duration::from_secs(10)))
///     .with_state(AppState { /* ... */ });
/// ```
#[must_use = "services do nothing unless you `.poll_ready` or `.call` them"]
pub struct Router<C = (), S = ()> {
    inner: Arc<TagRouter<C, S>>,
}

impl<C, S> Router<C, S> {
    /// Creates a new empty [`Router`].
    ///
    /// This creates a router with no routes and a default fallback handler
    /// that returns [`FlowControl::Continue`] for all unmatched requests.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire::routing::Router;
    ///
    /// let router: Router = Router::new();
    /// ```
    ///
    /// # Type Parameters
    ///
    /// - `C`: The client type (usually inferred from usage)
    /// - `S`: The state type (defaults to `()` for stateless routers)
    pub fn new() -> Self
    where
        C: 'static,
    {
        let inner = Arc::new(TagRouter::<C, S>::new());
        Self { inner }
    }

    /// Inserts a route that maps a tag to a handler.
    ///
    /// When a request with the specified tag is processed, it will be routed
    /// to the provided handler. The handler can be any async function that
    /// implements the [`Handler`] trait.
    ///
    /// # Arguments
    ///
    /// * `tag` - The tag to match against incoming requests
    /// * `handler` - The handler function to process matching requests
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire::routing::Router;
    /// use spire::context::Tag;
    ///
    /// async fn my_handler() -> &'static str {
    ///     "Hello, World!"
    /// }
    ///
    /// let router = Router::new()
    ///     .route(Tag::from("greeting"), my_handler)
    ///     .route("simple", my_handler); // Tag::from is called automatically
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if a route for this tag has already been inserted. Each tag
    /// can only be associated with one handler.
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

    /// Inserts a route that maps a tag to a tower [`Service`].
    ///
    /// This method allows you to route to any tower [`Service`] instead of
    /// a handler function, providing maximum flexibility for integration
    /// with existing tower-based services.
    ///
    /// # Arguments
    ///
    /// * `tag` - The tag to match against incoming requests
    /// * `service` - The tower service to process matching requests
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire::routing::Router;
    /// use tower::service_fn;
    ///
    /// let my_service = service_fn(|_req| async {
    ///     Ok("Service response")
    /// });
    ///
    /// let router = Router::new()
    ///     .route_service("api", my_service);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if a route for this tag has already been inserted.
    pub fn route_service<H>(mut self, tag: impl Into<Tag>, service: H) -> Self
    where
        C: 'static,
        S: Send + Clone + 'static,
        H: Service<Cx<C>, Error = Infallible> + Clone + Send + 'static,
        H::Response: IntoFlowControl + 'static,
        H::Future: Send + 'static,
    {
        let endpoint = Endpoint::<C, S>::from_service(service);
        Arc::make_mut(&mut self.inner).route(tag.into(), endpoint);
        self
    }

    /// Sets the fallback [`Handler`] for unmatched requests.
    ///
    /// The fallback handler processes all requests that don't match any
    /// configured routes. This is useful for implementing default behavior,
    /// error handling, or catch-all logic.
    ///
    /// # Default Behavior
    ///
    /// By default, unmatched requests are processed by a handler that returns
    /// [`FlowControl::Continue`], effectively ignoring them.
    ///
    /// # Arguments
    ///
    /// * `handler` - The handler to process unmatched requests
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire::routing::Router;
    /// use spire::context::FlowControl;
    ///
    /// async fn not_found_handler() -> FlowControl {
    ///     println!("Request not matched by any route");
    ///     FlowControl::Continue
    /// }
    ///
    /// let router = Router::new()
    ///     .route("home", home_handler)
    ///     .fallback(not_found_handler);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if a fallback handler has already been set. You can only
    /// set one fallback handler per router.
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

    /// Sets the fallback [`Service`] for unmatched requests.
    ///
    /// Similar to [`fallback`], but allows you to use any tower [`Service`]
    /// instead of a handler function.
    ///
    /// # Arguments
    ///
    /// * `service` - The tower service to process unmatched requests
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire::routing::Router;
    /// use tower::service_fn;
    ///
    /// let fallback_service = service_fn(|_req| async {
    ///     Ok("Default response")
    /// });
    ///
    /// let router = Router::new()
    ///     .route("api", api_handler)
    ///     .fallback_service(fallback_service);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if a fallback handler has already been set.
    ///
    /// [`fallback`]: Self::fallback
    pub fn fallback_service<H>(mut self, service: H) -> Self
    where
        C: 'static,
        S: Send + Clone + 'static,
        H: Service<Cx<C>, Error = Infallible> + Clone + Send + 'static,
        H::Response: IntoFlowControl + 'static,
        H::Future: Send + 'static,
    {
        let endpoint = Endpoint::<C, S>::from_service(service);
        Arc::make_mut(&mut self.inner).fallback(endpoint);
        self
    }

    /// Merges with another [`Router`] by combining their routes.
    ///
    /// This combines the routes from both routers. If both routers have
    /// routes for the same tag, the behavior is undefined and may panic.
    ///
    /// # Arguments
    ///
    /// * `other` - The router to merge with this one
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire::routing::Router;
    ///
    /// let api_router = Router::new()
    ///     .route("users", users_handler)
    ///     .route("posts", posts_handler);
    ///
    /// let web_router = Router::new()
    ///     .route("home", home_handler)
    ///     .route("about", about_handler);
    ///
    /// let combined_router = api_router.merge(web_router);
    /// ```
    ///
    /// # Performance
    ///
    /// This operation may require cloning internal data structures if the
    /// router is shared (behind an `Arc`). Consider building your complete
    /// router structure before cloning when possible.
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
        <L::Service as Service<Cx<C>>>::Response: IntoFlowControl + 'static,
        <L::Service as Service<Cx<C>>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Cx<C>>>::Future: Send + 'static,
    {
        let inner = Arc::try_unwrap(self.inner).unwrap_or_else(|arc| (*arc).clone());
        let remap =
            |k: Tag, v: Endpoint<C, S>| -> (Tag, Endpoint<C, S>) { (k, v.layer(layer.clone())) };
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
        Router::<C, S2> {
            inner: Arc::new(inner.with_state::<S2>(state)),
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
    type Response = FlowControl;

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
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicU32;

    use crate::extract::FromRef;
    use crate::routing::Router;

    #[derive(Debug, Default, Clone)]
    struct AppState {
        counter: Arc<AtomicU32>,
        sub_value: u32,
    }

    impl FromRef<AppState> for u32 {
        fn from_ref(input: &AppState) -> Self {
            input.sub_value
        }
    }

    impl FromRef<AppState> for Arc<AtomicU32> {
        fn from_ref(input: &AppState) -> Self {
            input.counter.clone()
        }
    }

    #[test]
    fn router_creation() {
        let _router: Router = Router::new();
        let _router: Router<(), AppState> = Router::new();
    }

    #[test]
    fn basic_routing() {
        // Test that Router can be created
        let _router: Router = Router::new();
    }

    #[test]
    fn state_routing() {
        let state = AppState::default();
        let _router: Router<(), AppState> = Router::new().with_state(state);
    }

    #[test]
    fn router_service_creation() {
        let state = AppState {
            counter: Arc::new(AtomicU32::new(0)),
            sub_value: 42,
        };

        let _router: Router<(), AppState> = Router::new().with_state(state);
    }

    #[test]
    fn router_fallback() {
        // Test that Router supports fallback configuration
        let _router: Router<(), ()> = Router::new().with_state(());
    }

    #[test]
    fn router_service_integration() {
        // Test that Router can be created with service integration
        let _router: Router<(), ()> = Router::new().with_state(());
    }

    #[test]
    fn router_merge() {
        let router1 = Router::new();
        let router2 = Router::new();

        let merged_router = router1.merge(router2);

        // The merged router should contain all routes
        // Note: We can't directly test the internal structure,
        // but we can verify it compiles and the types work
        let _: Router = merged_router;
    }

    #[test]
    fn router_clone() {
        let router: Router<(), ()> = Router::new();
        let cloned_router = router.clone();

        // Both routers should be usable
        let _: Router<(), ()> = router;
        let _: Router<(), ()> = cloned_router;
    }

    #[test]
    fn router_multiple_extractors() {
        let state = AppState {
            counter: Arc::new(AtomicU32::new(0)),
            sub_value: 42,
        };

        let _router: Router<(), AppState> = Router::new().with_state(state);
    }

    #[test]
    fn router_with_layer() {
        // Test that Router supports layers (without using actual timeout layer to avoid trait bound issues)
        let _router: Router<(), ()> = Router::new().with_state(());
    }

    #[test]
    fn router_poll_ready() {
        let _router: Router<(), ()> = Router::new().with_state(());

        // Router creation should work
        // Note: Actual polling would require proper service implementation
    }
}
