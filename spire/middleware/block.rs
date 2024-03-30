use std::collections::HashSet;
use std::convert::Infallible;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use tower::{Layer, Service};

use crate::context::{Context as Cx, Signal, Tag};

#[derive(Clone)]
pub struct Block<S> {
    inner: S,
    // Fallback means all not-yet encountered tags.
    block: Arc<Mutex<HashSet<Tag>>>,
}

impl<S> Block<S> {
    /// Creates a new [`Block`] with a provided inner service.
    pub fn new(inner: S) -> Self {
        let block = Arc::new(Mutex::new(HashSet::new()));
        Self { inner, block }
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

    fn apply_block(&self, tag: Tag) {
        // TODO: Tracing.
    }

    fn find_block(&self, tag: &Tag) -> bool {
        let block = self.block.lock().unwrap();
        block.contains(tag)
    }
}

impl<S> fmt::Debug for Block<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Defer").finish_non_exhaustive()
    }
}

impl<B, S> Service<Cx<B>> for Block<S>
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
struct BlockLayer {}

impl BlockLayer {
    /// Creates a new [`BlockLayer`].
    pub fn new() -> Self {
        Self {}
    }
}

impl<S> Layer<S> for BlockLayer {
    type Service = Block<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Block::new(inner)
    }
}
