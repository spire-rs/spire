use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use tower::{Layer, Service};

use crate::context::{Context as Cx, Signal, Tag, TagQuery};

pub struct Defer<S> {
    inner: S,
    // Fallback means all not-yet encountered tags.
    defer: Arc<Mutex<HashMap<Tag, Instant>>>,
}

impl<S> Defer<S> {
    /// Creates a new [`Defer`] with a provided inner service.
    pub fn new(inner: S) -> Self {
        let defer = Arc::new(Mutex::new(HashMap::new()));
        Self { inner, defer }
    }

    /// Returns a reference to the inner service.
    pub fn get_ref(&self) -> &S {
        &self.inner
    }

    /// Returns a mutable reference to the inner service.
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.inner
    }

    /// Returns the inner service, consuming self.
    pub fn into_inner(self) -> S {
        self.inner
    }

    fn apply_defer(&self, owner: Tag, query: TagQuery, duration: Duration) {
        let now = Instant::now();

        // TODO: Tracing.
        let defer = self.defer.lock().unwrap();
        match query {
            TagQuery::Owner => {}
            TagQuery::Single(_) => {}

            TagQuery::Every => {}
            TagQuery::List(_) => {}
        }

        todo!()
    }

    fn find_defer(&self, owner: &Tag) -> Instant {
        let now = Instant::now();

        let defer = self.defer.lock().unwrap();
        let until = match defer.get(owner).cloned() {
            None => defer.get(&Tag::Fallback).cloned(),
            Some(x) => Some(x),
        };

        until.unwrap_or(now)
    }
}

impl<S> fmt::Debug for Defer<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Defer").finish_non_exhaustive()
    }
}

impl<B, S> Service<Cx<B>> for Defer<S>
where
    S: Service<Cx<B>, Response = Signal, Error = Infallible> + Clone,
{
    type Response = Signal;
    type Error = Infallible;
    type Future = S::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        todo!()
    }

    #[inline]
    fn call(&mut self, cx: Cx<B>) -> Self::Future {
        todo!()
    }
}

#[derive(Debug, Clone)]
struct DeferLayer {}

impl DeferLayer {
    /// Creates a new [`BlockLayer`].
    pub fn new() -> Self {
        Self {}
    }
}

impl<S> Layer<S> for DeferLayer {
    type Service = Defer<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Defer::new(inner)
    }
}
