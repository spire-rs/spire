use std::fmt;

use crate::context::{Depth, Request, Task};
use crate::dataset::util::BoxCloneDataset;
use crate::dataset::Dataset;
use crate::{Error, Result};

/// [`Request`] queue backed by the [`Dataset`].
#[derive(Clone)]
pub struct Queue {
    request: Request,
    inner: BoxCloneDataset<Request, Error>,
}

impl Queue {
    /// Creates a new [`Queue`].
    pub fn new(request: Request, inner: BoxCloneDataset<Request, Error>) -> Self {
        Self { request, inner }
    }

    /// Inserts another [`Request`] into the queue.
    pub async fn append(&self, request: Request) -> Result<()> {
        self.inner.add(request).await
    }

    /// Automatically increases [`Request`]'s depth.
    pub async fn branch(&self, mut request: Request) -> Result<()> {
        let depth = self.request.depth().saturating_add(1);
        let f = || Depth::new(depth);
        let _ = request.extensions_mut().get_or_insert_with(f);
        self.append(request).await
    }
}

impl fmt::Debug for Queue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Queue").finish_non_exhaustive()
    }
}
