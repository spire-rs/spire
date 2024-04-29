use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt;
use std::task::{Context, Poll};

use tower::Service;

use spire_core::context::{Context as Cx, Signal, Tag, Task};

use crate::routing::{Endpoint, RouteFuture};

pub struct TagRouter<B, S> {
    endpoints: HashMap<Tag, Endpoint<B, S>>,
    current_fallback: Option<Endpoint<B, S>>,
    default_fallback: Endpoint<B, ()>,
}

impl<B, S> TagRouter<B, S> {
    /// Creates a new [`TagRouter`].
    pub fn new() -> Self
    where
        B: 'static,
    {
        Self {
            endpoints: HashMap::default(),
            current_fallback: None,
            default_fallback: Endpoint::default(),
        }
    }

    pub fn route(&mut self, tag: Tag, endpoint: Endpoint<B, S>) {
        if tag.is_fallback() {
            self.fallback(endpoint);
        } else if self.endpoints.insert(tag, endpoint).is_some() {
            panic!("should not override already routed tags")
        }
    }

    pub fn fallback(&mut self, endpoint: Endpoint<B, S>) {
        if self.current_fallback.replace(endpoint).is_some() {
            panic!("should not override already routed tags")
        }
    }

    pub fn layer<F>(mut self, func: F) -> Self
    where
        F: Fn(Tag, Endpoint<B, S>) -> (Tag, Endpoint<B, S>),
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

    pub fn with_state<S2>(self, state: S) -> TagRouter<B, S2>
    where
        S: Clone,
    {
        let remap = |(k, v): (Tag, Endpoint<B, S>)| (k, v.with_state(state.clone()));
        TagRouter {
            endpoints: self.endpoints.into_iter().map(remap).collect(),
            current_fallback: self.current_fallback.map(|x| x.with_state(state)),
            default_fallback: self.default_fallback,
        }
    }
}

impl<B, S> Clone for TagRouter<B, S> {
    fn clone(&self) -> Self {
        Self {
            endpoints: self.endpoints.clone(),
            current_fallback: self.current_fallback.clone(),
            default_fallback: self.default_fallback.clone(),
        }
    }
}

impl<B, S> fmt::Debug for TagRouter<B, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TagRouter").finish_non_exhaustive()
    }
}

impl<B> Service<Cx<B>> for TagRouter<B, ()> {
    type Response = Signal;
    type Error = Infallible;
    type Future = RouteFuture<B, Infallible>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, cx: Cx<B>) -> Self::Future {
        let fallback = || match &self.current_fallback {
            Some(user_fallback) => user_fallback.clone(),
            None => self.default_fallback.clone(),
        };

        let tagged = self.endpoints.get(cx.get_ref().tag()).cloned();
        let mut endpoint = tagged.unwrap_or_else(fallback);
        endpoint.call(cx)
    }
}
