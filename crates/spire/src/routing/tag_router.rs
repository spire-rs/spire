use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt;
use std::task::{Context, Poll};

use tower::Service;

use crate::context::{Context as Cx, FlowControl, Tag, TaskExt};
use crate::routing::{Endpoint, RouteFuture};

/// Routes [`Context`]s according to [`Tag`]s associated with a [`Request`].
///
/// [`Request`]: crate::context::Request
#[must_use = "services do nothing unless you `.poll_ready` or `.call` them"]
pub struct TagRouter<C, S> {
    endpoints: HashMap<Tag, Endpoint<C, S>>,
    fallback: Option<Endpoint<C, S>>,
}

impl<C, S> TagRouter<C, S> {
    /// Creates a new [`TagRouter`].
    pub fn new() -> Self
    where
        C: 'static,
    {
        Self {
            endpoints: HashMap::default(),
            fallback: None,
        }
    }

    /// Inserts a routed endpoint.
    ///
    /// # Panics
    ///
    /// Panics if a route for this tag has already been inserted.
    pub fn route(&mut self, tag: Tag, endpoint: Endpoint<C, S>) {
        if tag.is_fallback() {
            self.fallback(endpoint);
            return;
        }

        let tag_copy = tag.clone();
        if self.endpoints.insert(tag, endpoint).is_some() {
            panic!(
                "route conflict: tag '{:?}' has already been registered. \
                 Each tag can only be used once per router.",
                tag_copy
            );
        }
    }

    /// Inserts a fallback endpoint.
    ///
    /// # Panics
    ///
    /// Panics if a fallback handler has already been set.
    pub fn fallback(&mut self, endpoint: Endpoint<C, S>) {
        if self.fallback.replace(endpoint).is_some() {
            panic!(
                "fallback conflict: a fallback handler has already been set. \
                 Use Router::merge() to combine routers instead of setting multiple fallbacks."
            );
        }
    }

    /// Applies a transformation function to all endpoints.
    ///
    /// The function receives each `(Tag, Endpoint)` pair and returns a transformed pair.
    /// This is useful for applying middleware or transformations to all routes at once.
    pub fn map<F>(mut self, func: F) -> Self
    where
        F: Fn(Tag, Endpoint<C, S>) -> (Tag, Endpoint<C, S>),
    {
        // TODO??
        let it = self.endpoints.into_iter();
        self.endpoints = it.map(|(k, v)| func(k, v)).collect();
        self
    }

    /// Merges another router's routes into this one.
    ///
    /// All routes and the fallback handler from `other` are added to this router.
    ///
    /// # Panics
    ///
    /// Panics if any tags from `other` conflict with existing routes in this router.
    pub fn merge(&mut self, other: Self) {
        if let Some(endpoint) = other.fallback {
            self.fallback(endpoint);
        }

        for (tag, endpoint) in other.endpoints {
            self.route(tag, endpoint);
        }
    }

    /// Attaches state to all handlers in the router.
    ///
    /// Converts handlers from requiring state `S` to state `S2` by providing
    /// the state value. This allows stateless routers to become stateful.
    pub fn with_state<S2>(self, state: S) -> TagRouter<C, S2>
    where
        S: Clone,
    {
        let remap = |(k, v): (Tag, Endpoint<C, S>)| (k, v.with_state(state.clone()));
        TagRouter {
            endpoints: self.endpoints.into_iter().map(remap).collect(),
            fallback: self.fallback.map(|x| x.with_state(state)),
        }
    }
}

impl<C> TagRouter<C, ()>
where
    C: 'static,
{
    fn route_cloned(&self, tag: &Tag) -> Option<Endpoint<C, ()>> {
        self.endpoints.get(tag).cloned()
    }

    fn fallback_cloned(&self) -> Endpoint<C, ()> {
        self.fallback
            .as_ref()
            .map_or_else(Endpoint::default, Clone::clone)
    }
}

impl<C, S> Clone for TagRouter<C, S> {
    fn clone(&self) -> Self {
        Self {
            endpoints: self.endpoints.clone(),
            fallback: self.fallback.clone(),
        }
    }
}

impl<C, S> fmt::Debug for TagRouter<C, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TagRouter").finish_non_exhaustive()
    }
}

impl<C> Service<Cx<C>> for TagRouter<C, ()>
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
        tracing::trace!(tag = ?cx.get_ref().tag(), "routing request");

        self.route_cloned(cx.get_ref().tag())
            .unwrap_or_else(|| self.fallback_cloned())
            .call(cx)
    }
}
