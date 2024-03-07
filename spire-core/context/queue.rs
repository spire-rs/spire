use std::fmt;

use crate::context::Request;
use crate::dataset::{BoxDataset, Dataset, Result};

#[derive(Clone)]
pub struct Queue {
    inner: BoxDataset<Request>,
}

impl Queue {
    /// Creates a new [`Queue`].
    pub fn new<T>(dataset: T) -> Self
    where
        T: Dataset<Request>,
    {
        let inner = BoxDataset::new(dataset);
        Self { inner }
    }

    pub async fn append(&self, request: impl Into<Request>) -> Result<()> {
        self.inner.append(request.into()).await
    }

    // pub(crate) async fn poll(&self) -> Option<Request> { todo!() }
}

impl fmt::Debug for Queue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Queue").finish_non_exhaustive()
    }
}
