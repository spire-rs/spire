use std::fmt;

use crate::{BoxError, Error, Result};
use crate::context::Request;
use crate::dataset::{Dataset, InMemDataset};
use crate::dataset::util::{BoxCloneDataset, DatasetExt};

/// [`Request`] queue backed by the [`Dataset`].
#[derive(Clone)]
pub struct Queue(BoxCloneDataset<Request, Error>);

impl Queue {
    /// Creates a new [`Queue`].
    pub fn new<T, E>(dataset: T) -> Self
    where
        T: Dataset<Request, Error = E> + Clone,
        E: Into<BoxError>,
    {
        let f = |x: E| -> Error { Error::Dataset(x.into()) };
        Self(BoxCloneDataset::new(dataset.map_err(f)))
    }

    /// Inserts another request into the queue.
    pub async fn add(&self, request: Request) -> Result<()> {
        self.0.add(request).await
    }

    /// TODO. append, store current req in queue
    pub async fn branch(&self, request: Request) -> Result<()> {
        todo!()
    }

    /// Returns the next request from the queue.
    pub(crate) async fn get(&self) -> Result<Option<Request>> {
        self.0.get().await
    }
}

impl fmt::Debug for Queue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Queue").finish_non_exhaustive()
    }
}

impl Default for Queue {
    fn default() -> Self {
        Self::new(InMemDataset::queue())
    }
}
