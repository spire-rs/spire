//! [`Request`]'s [`Context`] and its extensions.
//!

use std::fmt;

pub use body::{Body, Request, Response};
use extend::Depth;
pub use extend::{Tag, Task, TaskBuilder};
pub use queue::RequestQueue;
pub use signal::{IntoSignal, Signal, TagQuery};

use crate::backend::Client;
use crate::dataset::util::BoxCloneDataset;
use crate::dataset::Datasets;
use crate::{Error, Result};

mod body;
mod extend;
mod queue;
mod signal;

/// Framework-specific context of the [`Request`].
pub struct Context<C> {
    request: Request,
    client: C,
    datasets: Datasets,
}

impl<C> Context<C>
where
    C: Client,
{
    /// Creates a new [`Context`].
    pub fn new(request: Request, client: C, datasets: Datasets) -> Self {
        Self {
            request,
            client,
            datasets,
        }
    }

    /// Resolves the [`Request`] and returns [`Response`] or [`Error`].
    pub async fn resolve(self) -> Result<Response> {
        let response = self.client.resolve(self.request).await?;
        Ok(response)
    }

    /// Returns the [`Backend`]'s client.
    ///
    /// [`Backend`]: crate::backend::Backend
    pub fn client(&self) -> C
    where
        C: Clone,
    {
        self.client.clone()
    }
}

impl<C> Context<C> {
    /// Returns the reference to the inner [`Request`].
    ///
    /// Used by extractors to access extensions.
    pub fn get_ref(&self) -> &Request {
        &self.request
    }

    /// Returns the mutable reference to the inner [`Request`].
    ///
    /// Used by extractors to access extensions.
    pub fn get_mut(&mut self) -> &mut Request {
        &mut self.request
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
