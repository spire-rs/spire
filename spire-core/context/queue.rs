use std::fmt;

use crate::BoxError;
use crate::context::Request;
use crate::dataset::{Dataset, InMemDataset};
use crate::dataset::util::{BoxCloneDataset, MapErr};

/// [`Request`] queue backed by the [`Dataset`].
#[derive(Clone)]
pub struct Queue {
    inner: BoxCloneDataset<Request, BoxError>,
}

impl Queue {
    /// Creates a new [`Queue`].
    pub fn new<T, E>(dataset: T) -> Self
    where
        T: Dataset<Request, Error = E> + Clone,
        E: Into<BoxError>,
    {
        let f = |x: E| -> BoxError { x.into() };
        let inner = BoxCloneDataset::new(MapErr::new(dataset, f));
        Self { inner }
    }

    pub async fn append(&self, request: impl Into<Request>) -> Result<(), BoxError> {
        // TODO: Make sure has event timestamp.
        self.inner.add(request.into()).await
    }

    pub(crate) async fn poll(&self) -> Result<Option<Request>, BoxError> {
        self.inner.get().await
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
