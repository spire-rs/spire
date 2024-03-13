//! [`Request`]'s [`Context`] and its extensions.
//!

use std::fmt;

use tower::{Service, ServiceExt};

pub use body::{Body, Request, Response};
pub use extend::{Tag, Task, TaskBuilder};
pub use queue::RequestQueue;
pub use signal::{IntoSignal, Query, Signal};

use crate::dataset::util::BoxCloneDataset;
use crate::dataset::Datasets;
use crate::{Error, Result};

mod body;
mod extend;
mod queue;
mod signal;

// TODO.
/// Framework-specific context of the task, [`Request`].
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

    pub fn peek(&self) -> &Request {
        &self.request
    }

    pub async fn try_resolve(mut self) -> Result<Response>
    where
        B: Service<Request, Response = Response, Error = Error>,
        <B as Service<Request>>::Future: Send,
    {
        let ret = self.backend.call(self.request).await;
        ret.map_err(Error::new)
    }

    pub fn queue(&self) -> RequestQueue {
        let dataset = self.datasets.get::<Request>();
        RequestQueue::new(dataset, self.request.depth())
    }

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
