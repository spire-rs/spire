//! Request queue for managing web scraping tasks.
//!
//! This module provides the [`RequestQueue`] type for managing HTTP requests
//! in a web scraping pipeline.

use std::fmt;

use crate::context::{Depth, Request, Tag};
use crate::dataset::Dataset;
use crate::dataset::utils::BoxCloneDataset;
use crate::{Error, Result};

/// [`Request`] queue backed by a [`Dataset`].
///
/// The queue manages requests and their metadata (tags, depth) for processing
/// by the scraping engine.
///
/// See [`Client::with_request_queue`].
///
/// [`Dataset`]: crate::dataset::Dataset
/// [`Client::with_request_queue`]: crate::Client::with_request_queue
#[must_use]
#[derive(Clone)]
pub struct RequestQueue {
    inner: BoxCloneDataset<Request, Error>,
    depth: usize,
}

impl RequestQueue {
    /// Creates a new [`RequestQueue`].
    pub const fn new(inner: BoxCloneDataset<Request, Error>, depth: usize) -> Self {
        Self { inner, depth }
    }

    /// Inserts another [`Request`] into the queue.
    pub async fn append(&self, mut request: Request) -> Result<()> {
        let _ = request.extensions_mut().get_or_insert_with(Tag::default);
        let _ = request.extensions_mut().get_or_insert_with(Depth::default);
        self.inner.write(request).await
    }

    /// Inserts another [`Request`] into the queue. Also increases `Request`'s depth.
    pub async fn branch(&self, mut request: Request) -> Result<()> {
        let depth = || Depth::new(self.depth.saturating_add(1));
        let _ = request.extensions_mut().get_or_insert_with(depth);
        self.append(request).await
    }
}

impl fmt::Debug for RequestQueue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RequestQueue").finish_non_exhaustive()
    }
}
