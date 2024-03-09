use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt;
use std::task::{Context, Poll};

use tower::Service;

use spire_core::backend::Backend;
use spire_core::context::{Context as Cx, Tag};
use spire_core::context::{Signal, Task};

use crate::routing::{Endpoint, RouteFuture};

pub struct TagRouter<B, S> {
    tag_router: HashMap<Tag, Endpoint<B, S>>,
    current_fallback: Option<Endpoint<B, S>>,
    default_fallback: Endpoint<B, ()>,
}

/// Ignores all incoming tasks by returning [`Signal::Continue`].
async fn default_fallback() -> Signal {
    Signal::Continue
}

impl<B, S> TagRouter<B, S> {
    /// Creates a new [`TagRouter`].
    pub fn new() -> Self
    where
        B: Backend,
    {
        Self {
            tag_router: HashMap::default(),
            current_fallback: None,
            default_fallback: Endpoint::from_handler(default_fallback),
        }
    }

    // TODO: Append instead?
    pub fn route(&mut self, tag: Tag, endpoint: Endpoint<B, S>) {
        if matches!(tag, Tag::Fallback) {
            self.fallback(endpoint);
        } else {
            let _ = self.tag_router.insert(tag, endpoint);
        }
    }

    pub fn fallback(&mut self, endpoint: Endpoint<B, S>) {
        self.current_fallback.replace(endpoint);
    }

    pub fn layer<F>(mut self, func: F) -> Self
    where
        F: Fn(Tag, Endpoint<B, S>) -> (Tag, Endpoint<B, S>),
    {
        let it = self.tag_router.into_iter();
        self.tag_router = it.map(|(k, v)| func(k, v)).collect();
        self
    }

    pub fn merge(&mut self, other: TagRouter<B, S>) -> Self {
        todo!()
    }

    pub fn with_state<S2>(self, state: S) -> TagRouter<B, S2>
    where
        S: Clone,
    {
        let remap = |(k, v): (Tag, Endpoint<B, S>)| (k, v.with_state(state.clone()));
        TagRouter {
            tag_router: self.tag_router.into_iter().map(remap).collect(),
            current_fallback: self.current_fallback.map(|x| x.with_state(state)),
            default_fallback: self.default_fallback,
        }
    }
}

impl<B, S> Clone for TagRouter<B, S> {
    fn clone(&self) -> Self {
        todo!()
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

        let tag = cx.request().tag().unwrap_or(&Tag::Fallback);
        let tagged = self.tag_router.get(tag).cloned();
        let mut endpoint = tagged.unwrap_or_else(fallback);
        endpoint.call(cx)
    }
}
