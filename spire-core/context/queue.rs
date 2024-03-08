use std::fmt;

use crate::context::Request;
use crate::dataset::util::{BoxCloneDataset, DatasetExt};
use crate::dataset::{Dataset, InMemDataset};
use crate::BoxError;

/// [`Request`] queue backed by the [`Dataset`].
#[derive(Clone)]
pub struct Queue(BoxCloneDataset<Request, BoxError>);

impl Queue {
    /// Creates a new [`Queue`].
    pub fn new<T, E>(dataset: T) -> Self
    where
        T: Dataset<Request, Error = E> + Clone,
        E: Into<BoxError>,
    {
        let f = |x: E| -> BoxError { x.into() };
        Self(BoxCloneDataset::new(dataset.map_err(f)))
    }
}

impl fmt::Debug for Queue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Queue").finish_non_exhaustive()
    }
}

impl Default for Queue {
    fn default() -> Self {
        Self::new(InMemDataset::fifo())
    }
}

#[async_trait::async_trait]
impl Dataset<Request> for Queue {
    type Error = BoxError;

    async fn add(&self, data: Request) -> Result<(), Self::Error> {
        self.0.add(data).await
    }

    async fn get(&self) -> Result<Option<Request>, Self::Error> {
        self.0.get().await
    }

    fn len(&self) -> usize {
        self.0.len()
    }
}
