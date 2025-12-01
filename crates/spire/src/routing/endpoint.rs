//! Internal endpoint abstraction for routing.
//!
//! This module provides the [`Endpoint`] enum which abstracts over different
//! types of request handlers in the routing system. An endpoint can be either
//! a direct [`Service`] or a [`Handler`] function that needs to be converted
//! to a service with state.
//!
//! # Design
//!
//! The [`Endpoint`] enum allows the router to treat both raw tower services
//! and Spire handler functions uniformly. This abstraction enables:
//!
//! - Mixing different handler types in the same router
//! - Applying middleware layers uniformly across all endpoint types
//! - State management for handler functions
//! - Efficient service cloning and execution
//!
//! # Internal Use Only
//!
//! This module is internal to the routing system and should not be used
//! directly by end users. The public [`Router`] API handles endpoint
//! creation and management automatically.
//!
//! [`Router`]: super::Router

use std::convert::Infallible;
use std::fmt;
use std::task::{Context, Poll};

use tower::{Layer, Service};

use crate::context::{Context as Cx, FlowControl, IntoFlowControl};
use crate::handler::Handler;
use crate::routing::make_route::MakeRoute;
use crate::routing::{Route, RouteFuture};

/// Internal abstraction over different types of request handlers.
///
/// An [`Endpoint`] represents either a tower [`Service`] or a Spire [`Handler`]
/// function within the routing system. This abstraction allows the router to
/// treat different handler types uniformly while maintaining their specific
/// characteristics.
///
/// # Variants
///
/// - [`Route`](Self::Route) - A ready-to-use tower service
/// - [`Handler`](Self::Handler) - A handler function that needs state to become a service
///
/// # Type Parameters
///
/// - `C` - The client/context type that handlers operate on
/// - `S` - The state type that handler functions require
pub enum Endpoint<C, S> {
    /// A ready-to-use tower [`Service`] that can be called directly.
    ///
    /// This variant contains services that have already been converted to the
    /// common [`Route`] interface and don't require additional state to function.
    Route(Route<C, Infallible>),

    /// A [`Handler`] function that requires state to become a service.
    ///
    /// This variant contains handler functions wrapped in [`MakeRoute`] that
    /// need to be combined with state before they can be used as services.
    Handler(MakeRoute<C, S, Infallible>),
}

impl<C, S> Endpoint<C, S> {
    /// Creates an [`Endpoint`] from a tower [`Service`].
    ///
    /// This method wraps a tower service in the [`Route`](Self::Route) variant,
    /// making it ready for use in the routing system. The service must be
    /// cloneable and handle the appropriate context type.
    ///
    /// # Arguments
    ///
    /// * `service` - A tower service that implements the required traits
    ///
    /// # Type Requirements
    ///
    /// The service must:
    /// - Accept [`Context<C>`](crate::context::Context) as its request type
    /// - Return responses that implement [`IntoFlowControl`]
    /// - Have an error type of [`Infallible`]
    /// - Be [`Clone`], [`Send`], and `'static`
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use tower::service_fn;
    /// use spire::routing::endpoint::Endpoint;
    ///
    /// let service = service_fn(|_req| async {
    ///     Ok("Hello, World!")
    /// });
    ///
    /// let endpoint = Endpoint::from_service(service);
    /// ```
    pub fn from_service<T>(service: T) -> Self
    where
        T: Service<Cx<C>, Error = Infallible> + Clone + Send + 'static,
        T::Response: IntoFlowControl + 'static,
        T::Future: Send + 'static,
    {
        Self::Route(Route::new(service))
    }

    /// Creates an [`Endpoint`] from a [`Handler`] function.
    ///
    /// This method wraps a handler function in the [`Handler`](Self::Handler) variant,
    /// preparing it for use in the routing system. The handler will be converted
    /// to a service when combined with state using [`with_state`](Self::with_state).
    ///
    /// # Arguments
    ///
    /// * `handler` - A handler function that implements the [`Handler`] trait
    ///
    /// # Type Parameters
    ///
    /// * `H` - The handler function type
    /// * `V` - The handler's extractor type signature
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire::routing::endpoint::Endpoint;
    ///
    /// async fn my_handler() -> &'static str {
    ///     "Hello, World!"
    /// }
    ///
    /// let endpoint = Endpoint::from_handler(my_handler);
    /// ```
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

    /// Applies a tower [`Layer`] to this endpoint.
    ///
    /// This method wraps the endpoint with middleware, allowing you to add
    /// functionality like logging, rate limiting, or request transformation.
    /// The layer is applied uniformly regardless of the endpoint's internal type.
    ///
    /// # Arguments
    ///
    /// * `layer` - A tower layer to apply to the endpoint
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use tower::timeout::TimeoutLayer;
    /// use std::time::Duration;
    ///
    /// let endpoint = Endpoint::from_handler(my_handler)
    ///     .layer(TimeoutLayer::new(Duration::from_secs(30)));
    /// ```
    ///
    /// # Performance
    ///
    /// Applying layers creates a new service stack, which may have runtime
    /// overhead. Consider the performance implications when applying many layers.
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
        match self {
            Self::Route(x) => Self::Route(x.layer(layer)),
            Self::Handler(x) => Self::Handler(x.layer(layer)),
        }
    }

    /// Attaches state to the endpoint, converting it to a stateful endpoint.
    ///
    /// This method consumes the endpoint and produces a new endpoint with the
    /// specified state attached. For [`Handler`](Self::Handler) variants, this
    /// converts the handler into a ready-to-use service. For [`Route`](Self::Route)
    /// variants, the state is ignored since routes don't need additional state.
    ///
    /// # Arguments
    ///
    /// * `state` - The state to attach to handler functions
    ///
    /// # Type Transformation
    ///
    /// This method changes the state type parameter from `S` to `S2`, allowing
    /// you to transform stateless endpoints into stateful ones.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// #[derive(Clone)]
    /// struct AppState {
    ///     db: Database,
    /// }
    ///
    /// let endpoint = Endpoint::from_handler(my_handler)
    ///     .with_state(AppState { db: create_db() });
    /// ```
    pub fn with_state<S2>(self, state: S) -> Endpoint<C, S2> {
        match self {
            Self::Route(x) => Endpoint::Route(x),
            Self::Handler(x) => Endpoint::Route(x.into_route(state)),
        }
    }
}

/// Default fallback handler that ignores all incoming tasks.
///
/// This function serves as the default fallback behavior for unmatched routes
/// in the router. It simply returns [`FlowControl::Continue`], effectively
/// ignoring the request and allowing processing to continue with the next task.
///
/// # Behavior
///
/// - Accepts any request without processing it
/// - Always returns [`FlowControl::Continue`]
/// - Used when no explicit fallback handler is provided
/// - Ensures unmatched requests don't cause errors
///
/// # Performance
///
/// This handler has minimal overhead as it performs no processing and
/// immediately returns a continue signal.
async fn default_fallback() -> FlowControl {
    FlowControl::Continue
}

/// Default implementation for stateless endpoints.
///
/// Creates an endpoint using the [`default_fallback`] handler, which ignores
/// all incoming requests. This is used internally by the router when no
/// explicit fallback behavior is specified.
impl<C> Default for Endpoint<C, ()>
where
    C: 'static,
{
    /// Creates a default endpoint that ignores all requests.
    ///
    /// The default endpoint uses [`default_fallback`] as its handler,
    /// which returns [`FlowControl::Continue`] for all requests.
    #[inline]
    fn default() -> Self {
        Self::from_handler(default_fallback)
    }
}

/// Clone implementation for endpoints.
///
/// Cloning an endpoint creates a new instance that shares the same underlying
/// handler or service. This is efficient as the internal types are designed
/// to be cheaply cloneable.
impl<C, S> Clone for Endpoint<C, S> {
    /// Creates a clone of the endpoint.
    ///
    /// The cloning behavior depends on the endpoint variant:
    /// - [`Route`](Self::Route) variants clone the underlying route
    /// - [`Handler`](Self::Handler) variants clone the handler wrapper
    fn clone(&self) -> Self {
        match self {
            Self::Route(x) => Self::Route(x.clone()),
            Self::Handler(x) => Self::Handler(x.clone()),
        }
    }
}

/// Debug implementation for endpoints.
///
/// Provides debug formatting that delegates to the underlying route or
/// handler's debug implementation, maintaining useful debugging information.
impl<C, S> fmt::Debug for Endpoint<C, S> {
    /// Formats the endpoint for debugging.
    ///
    /// The debug output reflects the endpoint's internal structure:
    /// - [`Route`](Self::Route) variants show route debug information
    /// - [`Handler`](Self::Handler) variants show handler debug information
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Route(x) => x.fmt(f),
            Self::Handler(x) => x.fmt(f),
        }
    }
}

/// Tower service implementation for stateless endpoints.
///
/// This allows stateless endpoints to be used directly as tower services.
/// The implementation handles both route and handler variants transparently,
/// converting handler variants to routes with unit state when called.
impl<C> Service<Cx<C>> for Endpoint<C, ()> {
    type Error = Infallible;
    type Future = RouteFuture<C, Infallible>;
    type Response = FlowControl;

    /// Checks if the endpoint is ready to accept requests.
    ///
    /// Endpoints are always ready since they don't maintain any internal
    /// state that could cause backpressure.
    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    /// Executes the endpoint with the given context.
    ///
    /// This method handles the execution logic for both endpoint variants:
    /// - [`Route`](Self::Route) variants are called directly
    /// - [`Handler`](Self::Handler) variants are converted to routes with unit state first
    ///
    /// # Arguments
    ///
    /// * `cx` - The request context to process
    ///
    /// # Returns
    ///
    /// A future that resolves to a [`FlowControl`] indicating how processing should continue.
    #[inline]
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all, level = "trace"))]
    fn call(&mut self, cx: Cx<C>) -> Self::Future {
        #[cfg(feature = "tracing")]
        tracing::trace!(endpoint_type = ?match self {
            Self::Route(_) => "Route",
            Self::Handler(_) => "Handler",
        }, "executing endpoint");

        match self {
            Self::Route(x) => x.call(cx),
            Self::Handler(x) => x.clone().into_route(()).call(cx),
        }
    }
}
