use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt;
use std::task::{Context, Poll};

use tower::Service;

use crate::context::{Context as Cx, Signal, Tag, Task};
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
    /// # Errors
    ///
    /// Panics if overrides an already inserted route.
    pub fn route(&mut self, tag: Tag, endpoint: Endpoint<C, S>) {
        if tag.is_fallback() {
            self.fallback(endpoint);
            return;
        }

        assert!(
            self.endpoints.insert(tag, endpoint).is_none(),
            "should not override already routed tags"
        );
    }

    /// Inserts a fallback endpoint.
    ///
    /// # Errors
    ///
    /// Panics if overrides an already inserted fallback.
    pub fn fallback(&mut self, endpoint: Endpoint<C, S>) {
        assert!(
            self.fallback.replace(endpoint).is_none(),
            "should not override fallback route"
        );
    }

    /// TODO.
    pub fn map<F>(mut self, func: F) -> Self
    where
        F: Fn(Tag, Endpoint<C, S>) -> (Tag, Endpoint<C, S>),
    {
        // TODO??
        let it = self.endpoints.into_iter();
        self.endpoints = it.map(|(k, v)| func(k, v)).collect();
        self
    }

    /// TODO.
    pub fn merge(&mut self, other: Self) {
        if let Some(endpoint) = other.fallback {
            self.fallback(endpoint);
        }

        for (tag, endpoint) in other.endpoints {
            self.route(tag, endpoint);
        }
    }

    /// TODO.
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
    type Response = Signal;
    type Error = Infallible;
    type Future = RouteFuture<C, Infallible>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, cx: Cx<C>) -> Self::Future {
        self.route_cloned(cx.get_ref().tag())
            .unwrap_or_else(|| self.fallback_cloned())
            .call(cx)
    }
}
