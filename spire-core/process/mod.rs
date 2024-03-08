use std::convert::Infallible;
use std::fmt;
use std::sync::Arc;

use tower::Service;

use crate::backend::Backend;
use crate::context::{Context, Queue, Request, Signal};
use crate::dataset::{BoxDataset, Dataset};

mod future;
mod metric;

pub struct Daemon<B, S> {
    inner: Arc<DaemonInner<B, S>>,
}

struct DaemonInner<B, S> {
    backend: B,
    inner: S,
    queue: Queue,
}

impl<B, S> Daemon<B, S> {
    /// Creates a new [`Daemon`].
    pub fn new(backend: B, inner: S) -> Self
    where
        B: Backend,
        S: Service<Context<B>, Response = Signal, Error = Infallible> + Clone,
    {
        let inner = Arc::new(DaemonInner {
            queue: Queue::default(),
            inner,
            backend,
        });

        Self { inner }
    }

    /// Replaces the [`Dataset`] use by the request queue.
    ///
    /// If the [`Dataset`] for the request queue is not provided, then
    /// the default [`Queue`], backed by [`InMemDataset`] is used instead.
    ///
    /// ### Note
    ///
    /// Does not move items from the replaced dataset.
    ///
    /// [`InMemDataset`]: crate::dataset::InMemDataset
    pub fn with_queue<D>(mut self, dataset: D) -> Self
    where
        B: Clone,
        S: Clone,
        D: Dataset<Request>,
    {
        self.map_inner(|mut inner| {
            inner.queue = Queue::new(dataset);
            inner
        })
    }

    pub fn with_dataset<D, T>(self, dataset: D) -> Self
    where
        D: Dataset<T>,
    {
        let dataset = BoxDataset::new(dataset);
        todo!()
    }

    fn map_inner<F>(self, f: F) -> Self
    where
        B: Clone,
        S: Clone,
        F: FnOnce(DaemonInner<B, S>) -> DaemonInner<B, S>,
    {
        let inner = Arc::new(f(self.into_inner()));
        Self { inner }
    }

    fn into_inner(self) -> DaemonInner<B, S>
    where
        B: Clone,
        S: Clone,
    {
        Arc::try_unwrap(self.inner).unwrap_or_else(|x| DaemonInner {
            inner: x.inner.clone(),
            backend: x.backend.clone(),
            queue: x.queue.clone(),
        })
    }

    async fn call_after_poll(&self) -> Signal
    where
        B: Clone,
    {
        if let Some(request) = self.inner.queue.poll().await {
            let backend = self.inner.backend.clone();
            let queue = self.inner.queue.clone();
            Context::new(backend, queue, request)
        } else {
            return Signal::Skip;
        };

        todo!()
    }

    pub async fn run(self)
    where
        S: Service<Context<B>, Response = Signal, Error = Infallible>,
    {
        todo!()
    }
}

impl<B, S> Clone for Daemon<B, S> {
    fn clone(&self) -> Self {
        let inner = self.inner.clone();
        Self { inner }
    }
}

impl<B, S> fmt::Debug for Daemon<B, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Daemon").finish_non_exhaustive()
    }
}
