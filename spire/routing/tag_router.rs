use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt;
use std::task::{Context, Poll};

use tower::Service;

use spire_core::context::{Context as Cx, Signal, Tag, Task};

use crate::routing::{Endpoint, RouteFuture};

pub struct TagRouter<C, S> {
    endpoints: HashMap<Tag, Endpoint<C, S>>,
    current_fallback: Option<Endpoint<C, S>>,
    default_fallback: Endpoint<C, ()>,
}

impl<C, S> TagRouter<C, S> {
    /// Creates a new [`TagRouter`].
    pub fn new() -> Self
    where
        C: 'static,
    {
        Self {
            endpoints: HashMap::default(),
            current_fallback: None,
            default_fallback: Endpoint::default(),
        }
    }

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

    pub fn fallback(&mut self, endpoint: Endpoint<C, S>) {
        assert!(
            self.current_fallback.replace(endpoint).is_none(),
            "should not override already routed tags"
        );
    }

    pub fn layer<F>(mut self, func: F) -> Self
    where
        F: Fn(Tag, Endpoint<C, S>) -> (Tag, Endpoint<C, S>),
    {
        let it = self.endpoints.into_iter();
        self.endpoints = it.map(|(k, v)| func(k, v)).collect();
        self
    }

    pub fn merge(&mut self, other: Self) {
        if let Some(x) = other.current_fallback {
            self.fallback(x);
        }

        for (tag, endpoint) in other.endpoints {
            self.route(tag, endpoint);
        }
    }

    pub fn with_state<S2>(self, state: S) -> TagRouter<C, S2>
    where
        S: Clone,
    {
        let remap = |(k, v): (Tag, Endpoint<C, S>)| (k, v.with_state(state.clone()));
        TagRouter {
            endpoints: self.endpoints.into_iter().map(remap).collect(),
            current_fallback: self.current_fallback.map(|x| x.with_state(state)),
            default_fallback: self.default_fallback,
        }
    }
}

impl<C, S> Clone for TagRouter<C, S> {
    fn clone(&self) -> Self {
        Self {
            endpoints: self.endpoints.clone(),
            current_fallback: self.current_fallback.clone(),
            default_fallback: self.default_fallback.clone(),
        }
    }
}

impl<C, S> fmt::Debug for TagRouter<C, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TagRouter").finish_non_exhaustive()
    }
}

impl<C> Service<Cx<C>> for TagRouter<C, ()> {
    type Response = Signal;
    type Error = Infallible;
    type Future = RouteFuture<C, Infallible>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, cx: Cx<C>) -> Self::Future {
        let fallback = || match &self.current_fallback {
            Some(user_fallback) => user_fallback.clone(),
            None => self.default_fallback.clone(),
        };

        let tagged = self.endpoints.get(cx.get_ref().tag()).cloned();
        let mut endpoint = tagged.unwrap_or_else(fallback);
        endpoint.call(cx)
    }
}
