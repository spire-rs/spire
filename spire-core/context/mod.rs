//! [`Request`]'s [`Context`] and its extensions.
//!

use std::fmt;

pub use body::{Body, Request, Response};
use extend::Depth;
pub use extend::{Tag, Task, TaskBuilder};
pub use queue::RequestQueue;
pub use signal::{IntoSignal, Signal, TagQuery};

use crate::backend::{Backend, Client};
use crate::dataset::util::BoxCloneDataset;
use crate::dataset::Datasets;
use crate::{Error, Result};

mod body;
mod extend;
mod queue;
mod signal;

/// Framework-specific context of the [`Request`].
pub struct Context<B> {
    request: Request,
    backend: B,
    datasets: Datasets,
}

impl<B> Context<B> {
    /// Creates a new [`Context`].
    pub fn new(request: Request, backend: B, datasets: Datasets) -> Self {
        Self {
            request,
            backend,
            datasets,
        }
    }

    /// Returns the reference to the inner [`Request`].
    ///
    /// Used by extractors to access extensions.
    pub fn peek(&self) -> &Request {
        &self.request
    }

    /// Resolves the [`Request`] and returns [`Response`] or [`Error`].
    pub async fn try_resolve(self) -> Result<Response>
    where
        B: Backend,
    {
        let client: B::Client = self.backend.call().await?;
        let response: Response = client.invoke(self.request).await?;
        Ok(response)
    }

    /// Returns the [`Backend`]'s client.
    pub async fn client(&self) -> Result<B::Client>
    where
        B: Backend,
    {
        self.backend.call().await
    }

    /// Initializes and returns the [`RequestQueue`].
    pub fn queue(&self) -> RequestQueue {
        let dataset = self.datasets.get::<Request>();
        RequestQueue::new(dataset, self.request.depth())
    }

    /// Initializes and returns the boxed [`Dataset`] of type `T`.
    ///
    /// [`Dataset`]: crate::dataset::Dataset
    pub fn dataset<T>(&self) -> BoxCloneDataset<T, Error>
    where
        T: Send + Sync + 'static,
    {
        self.datasets.get::<T>()
    }
}

impl<B> fmt::Debug for Context<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context").finish_non_exhaustive()
    }
}
