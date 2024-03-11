use std::fmt;

use crate::context::{Depth, Request};
use crate::dataset::util::BoxCloneDataset;
use crate::dataset::Dataset;
use crate::{Error, Result};

/// [`Request`] queue backed by the [`Dataset`]<`Request`>.
#[derive(Clone)]
pub struct Queue {
    inner: BoxCloneDataset<Request, Error>,
    depth: usize,
}

impl Queue {
    /// Creates a new [`Queue`].
    pub fn new(inner: BoxCloneDataset<Request, Error>, depth: usize) -> Self {
        Self { inner, depth }
    }

    /// Inserts another [`Request`] into the queue.
    pub async fn append(&self, request: Request) -> Result<()> {
        self.inner.add(request).await
    }

    /// Inserts another [`Request`] into the queue. Also increases `Request`'s depth.
    pub async fn branch(&self, mut request: Request) -> Result<()> {
        let depth = Depth::new(self.depth.saturating_add(1));
        let _ = request.extensions_mut().get_or_insert(depth);
        self.append(request).await
    }
}

impl fmt::Debug for Queue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Queue").finish_non_exhaustive()
    }
}
