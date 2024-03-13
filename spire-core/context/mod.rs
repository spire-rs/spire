//! [`Request`]'s [`Context`] and its extensions.
//!

use tower::{Service, ServiceExt};

pub use body::{Body, Request, Response};
pub use extend::{Tag, Task, TaskBuilder};
pub use queue::Queue;
pub use signal::{IntoSignal, Query, Signal};

use crate::{Error, Result};
use crate::backend::Backend;
use crate::dataset::Datasets;
use crate::dataset::util::BoxCloneDataset;

mod body;
mod extend;
mod queue;
mod signal;

// TODO.
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

    pub fn peek_request(&self) -> Option<&Request> {
        Some(&self.request)
    }

    pub async fn try_resolve(mut self) -> Result<Response>
    where
        B: Service<Request, Response = Response, Error = Error>,
        <B as Service<Request>>::Future: Send,
    {
        let resp = self.backend.call(self.request).await;
        resp.map_err(Error::new)
    }

    pub fn queue(&self) -> Queue {
        let dataset = self.datasets.get::<Request>();
        Queue::new(dataset, self.request.depth())
    }

    pub fn dataset<T>(&self) -> BoxCloneDataset<T, Error>
    where
        T: Send + Sync + 'static,
    {
        self.datasets.get::<T>()
    }
}
