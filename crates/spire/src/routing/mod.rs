//! Routing requests to handlers by their [`Tag`].

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt;
use std::future::Future;
use std::task::{Context as TaskContext, Poll};

use futures::FutureExt;
use futures::future::BoxFuture;
use tower::util::BoxCloneSyncService;
use tower::{Layer, Service, ServiceExt};

use crate::handler::Handler;
use crate::request::{RequestExt, Tag};
use crate::worker::{IntoOutcome, Outcome, Worker};
use crate::{Context, Error, TRACING_TARGET};

/// A type-erased route: a tower service from a [`Context`] to an [`Outcome`].
pub type Route<C> = BoxCloneSyncService<Context<C>, Outcome, std::convert::Infallible>;

/// Routes requests to handlers by their [`Tag`].
///
/// Tagged requests go to the handler registered for their tag; untagged
/// requests and tags without a handler go to the fallback. Without a
/// fallback, they fail, so a misspelled tag shows up in the crawl's stats.
///
/// State is given up front with [`with_state`](Self::with_state) and shared
/// by every handler through [`State`](crate::extract::State).
///
/// ```
/// use spire::Router;
/// use spire::extract::Text;
/// use spire::frontier::Queue;
///
/// async fn list(queue: Queue) -> spire::Result<()> {
///     queue.follow("/items/1").await?;
///     Ok(())
/// }
///
/// async fn item(Text(body): Text) {
///     println!("{} bytes", body.len());
/// }
///
/// let router: Router<spire::backend::Noop> = Router::new()
///     .route("list", list)
///     .route("item", item);
/// ```
#[must_use]
pub struct Router<C, S = ()> {
    routes: HashMap<Tag, Route<C>>,
    fallback: Option<Route<C>>,
    state: S,
}

impl<C> Router<C> {
    /// Creates a router without state.
    pub fn new() -> Self {
        Self::with_state(())
    }
}

impl<C, S> Router<C, S> {
    /// Creates a router whose handlers share `state`.
    pub fn with_state(state: S) -> Self {
        Self {
            routes: HashMap::new(),
            fallback: None,
            state,
        }
    }

    /// Routes requests tagged `tag` to `handler`.
    ///
    /// # Panics
    ///
    /// Panics if `tag` already has a route.
    pub fn route<H, M>(self, tag: impl Into<Tag>, handler: H) -> Self
    where
        C: Send + 'static,
        S: Clone + Send + Sync + 'static,
        H: Handler<C, M, S>,
        M: 'static,
    {
        let service = handler.with_state(self.state.clone());
        self.insert(tag.into(), Route::new(service))
    }

    /// Routes requests tagged `tag` to a tower service.
    ///
    /// # Panics
    ///
    /// Panics if `tag` already has a route.
    pub fn route_service<T>(self, tag: impl Into<Tag>, service: T) -> Self
    where
        C: Send + 'static,
        T: Service<Context<C>, Response: IntoOutcome, Error: IntoOutcome>,
        T: Clone + Send + Sync + 'static,
        T::Future: Send + 'static,
    {
        self.insert(tag.into(), erase(service))
    }

    /// Routes untagged requests, and tags without a route, to `handler`.
    pub fn fallback<H, M>(mut self, handler: H) -> Self
    where
        C: Send + 'static,
        S: Clone + Send + Sync + 'static,
        H: Handler<C, M, S>,
        M: 'static,
    {
        let service = handler.with_state(self.state.clone());
        self.fallback = Some(Route::new(service));
        self
    }

    /// Routes untagged requests, and tags without a route, to a tower
    /// service.
    pub fn fallback_service<T>(mut self, service: T) -> Self
    where
        C: Send + 'static,
        T: Service<Context<C>, Response: IntoOutcome, Error: IntoOutcome>,
        T: Clone + Send + Sync + 'static,
        T::Future: Send + 'static,
    {
        self.fallback = Some(erase(service));
        self
    }

    /// Adds the routes of `other`, keeping this router's state.
    ///
    /// # Panics
    ///
    /// Panics if both routers route the same tag or both have a fallback.
    pub fn merge(mut self, other: Self) -> Self {
        for (tag, route) in other.routes {
            self = self.insert(tag, route);
        }
        if let Some(fallback) = other.fallback {
            assert!(
                self.fallback.replace(fallback).is_none(),
                "both routers have a fallback"
            );
        }
        self
    }

    /// Wraps every route, and the fallback, in a tower [`Layer`].
    ///
    /// Routes added afterwards are not wrapped. The layer's errors become
    /// failed requests.
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use spire::Router;
    ///
    /// async fn handler() {}
    ///
    /// let router: Router<spire::backend::Noop> = Router::new()
    ///     .route("page", handler)
    ///     .layer(tower::timeout::TimeoutLayer::new(Duration::from_secs(30)));
    /// ```
    pub fn layer<L>(mut self, layer: L) -> Self
    where
        C: Send + 'static,
        L: Layer<Route<C>>,
        L::Service: Service<Context<C>, Response: IntoOutcome, Error: IntoOutcome>,
        L::Service: Clone + Send + Sync + 'static,
        <L::Service as Service<Context<C>>>::Future: Send + 'static,
    {
        let fallback = self.fallback.take().unwrap_or_else(|| Route::new(Unrouted));
        self.fallback = Some(erase(layer.layer(fallback)));
        self.routes = self
            .routes
            .into_iter()
            .map(|(tag, route)| (tag, erase(layer.layer(route))))
            .collect();
        self
    }

    fn insert(mut self, tag: Tag, route: Route<C>) -> Self {
        match self.routes.entry(tag) {
            Entry::Occupied(entry) => panic!("tag `{}` already has a route", entry.key()),
            Entry::Vacant(entry) => entry.insert(route),
        };
        self
    }

    /// Calls the route for the request: its tag's, or else the fallback.
    fn dispatch(&self, cx: Context<C>) -> impl Future<Output = Outcome> + Send + use<C, S>
    where
        C: Send + 'static,
    {
        let tag = cx.request().tag();
        tracing::trace!(
            target: TRACING_TARGET,
            tag = tag.map(tracing::field::display),
            "routing request",
        );

        let route = tag
            .and_then(|tag| self.routes.get(tag))
            .or(self.fallback.as_ref())
            .cloned();
        async move {
            let route = route.unwrap_or_else(|| Route::new(Unrouted));
            match route.oneshot(cx).await {
                Ok(outcome) => outcome,
                Err(never) => match never {},
            }
        }
    }
}

/// Erases a service into a [`Route`].
fn erase<C, T>(service: T) -> Route<C>
where
    C: Send + 'static,
    T: Service<Context<C>, Response: IntoOutcome, Error: IntoOutcome>,
    T: Clone + Send + Sync + 'static,
    T::Future: Send + 'static,
{
    Route::new(Outcomes(service))
}

/// Turns a service's responses and errors, including readiness errors, into
/// [`Outcome`]s.
#[derive(Clone)]
struct Outcomes<T>(T);

impl<C, T> Service<Context<C>> for Outcomes<T>
where
    C: Send + 'static,
    T: Service<Context<C>, Response: IntoOutcome, Error: IntoOutcome>,
    T: Clone + Send + 'static,
    T::Future: Send + 'static,
{
    type Error = std::convert::Infallible;
    type Future = BoxFuture<'static, Result<Outcome, Self::Error>>;
    type Response = Outcome;

    fn poll_ready(&mut self, _cx: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, cx: Context<C>) -> Self::Future {
        let service = self.0.clone();
        async move { Ok(service.oneshot(cx).await.into_outcome()) }.boxed()
    }
}

/// The default fallback: fails requests nothing routes.
#[derive(Debug, Clone, Copy)]
struct Unrouted;

impl<C> Service<Context<C>> for Unrouted {
    type Error = std::convert::Infallible;
    type Future = std::future::Ready<Result<Outcome, Self::Error>>;
    type Response = Outcome;

    fn poll_ready(&mut self, _cx: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, cx: Context<C>) -> Self::Future {
        let error = match cx.request().tag() {
            Some(tag) => Error::worker(format!("no route for tag `{tag}`")),
            None => Error::worker("no fallback route for an untagged request"),
        };
        std::future::ready(Ok(Outcome::Fail(error)))
    }
}

impl<C> Default for Router<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C, S: Clone> Clone for Router<C, S> {
    fn clone(&self) -> Self {
        Self {
            routes: self.routes.clone(),
            fallback: self.fallback.clone(),
            state: self.state.clone(),
        }
    }
}

impl<C, S> fmt::Debug for Router<C, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut tags: Vec<_> = self.routes.keys().map(Tag::as_str).collect();
        tags.sort_unstable();
        f.debug_struct("Router")
            .field("routes", &tags)
            .field("fallback", &self.fallback.is_some())
            .finish_non_exhaustive()
    }
}

impl<C, S> Worker<C> for Router<C, S>
where
    C: Send + 'static,
    S: Send + Sync + 'static,
{
    fn handle(&self, cx: Context<C>) -> impl Future<Output = Outcome> + Send {
        self.dispatch(cx)
    }
}

/// Routers nest: a router is a service usable as another router's route.
impl<C, S> Service<Context<C>> for Router<C, S>
where
    C: Send + 'static,
    S: Send + Sync + 'static,
{
    type Error = std::convert::Infallible;
    type Future = BoxFuture<'static, Result<Outcome, Self::Error>>;
    type Response = Outcome;

    fn poll_ready(&mut self, _cx: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, cx: Context<C>) -> Self::Future {
        self.dispatch(cx).map(Ok).boxed()
    }
}

#[cfg(test)]
mod tests;
