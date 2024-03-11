use std::convert::Infallible;
use std::fmt;
use std::sync::Arc;

use tower::Service;

use crate::backend::Backend;
use crate::context::{Context, Request, Signal};
use crate::dataset::util::BoxCloneDataset;
use crate::dataset::{Dataset, Datasets};
use crate::process::runner::Runner;
use crate::{BoxError, Error, Result};

mod future;
mod metric;
mod runner;

pub struct Daemon<B, S> {
    inner: Arc<Runner<B, S>>,
}

impl<B, S> Daemon<B, S> {
    /// Creates a new [`Daemon`].
    pub fn new(backend: B, inner: S) -> Self
    where
        B: Backend,
        S: Service<Context<B>, Response = Signal, Error = Infallible> + Clone,
    {
        let datasets = Datasets::default();
        let inner = Runner::new(backend, inner, datasets);
        Self {
            inner: Arc::new(inner),
        }
    }

    pub async fn run(self) -> Result<()>
    where
        S: Service<Context<B>, Response = Signal, Error = Infallible>,
    {
        todo!()
    }

    /// Replaces the [`Dataset`] used by the [`Queue`].
    ///
    /// If the `Dataset` for the `Queue` is not provided, then
    /// the queue backed by the [`InMemDataset`] is used instead.
    ///
    /// ### Note
    ///
    /// Does not move items from the replaced `Dataset`.
    ///
    /// [`InMemDataset`]: InMemDataset
    /// [`Queue`]: crate::context::Queue
    pub fn with_queue<D, E>(self, dataset: D) -> Self
    where
        D: Dataset<Request, Error = E> + Clone,
        E: Into<BoxError>,
    {
        self.inner.datasets.set(dataset);
        self
    }

    /// Inserts or replaces (if any already inserted) the provided [`Dataset`].
    ///
    /// ### Note
    ///
    /// If the handler requests for a [`Dataset`] of a specific type, but  no `Dataset` of this
    /// type was provided, it will be lazily initialized as a `first-in first-out` [`InMemDataset`].
    ///
    /// [`InMemDataset`]: InMemDataset
    pub fn with_dataset<D, E, T>(self, dataset: D) -> Self
    where
        D: Dataset<T, Error = E> + Clone,
        E: Into<BoxError>,
        T: Send + Sync + 'static,
    {
        self.inner.datasets.set(dataset);
        self
    }

    pub fn dataset<T>(&self) -> BoxCloneDataset<T, Error>
    where
        T: Send + Sync + 'static,
    {
        self.inner.datasets.get::<T>()
    }

    fn map_inner<F>(self, f: F) -> Self
    where
        B: Clone,
        S: Clone,
        F: FnOnce(Runner<B, S>) -> Runner<B, S>,
    {
        let inner = Arc::new(f(self.into_inner()));
        Self { inner }
    }

    fn into_inner(self) -> Runner<B, S>
    where
        B: Clone,
        S: Clone,
    {
        Arc::try_unwrap(self.inner).unwrap_or_else(|x| Runner {
            datasets: x.datasets.clone(),
            backend: x.backend.clone(),
            service: x.service.clone(),
        })
    }

    async fn call_after_poll(&self) -> Signal
    where
        B: Clone,
    {
        // let poll = self.inner.queue.get();
        // let cx = if let Ok(Some(request)) = poll.await {
        //     let backend = self.inner.backend.clone();
        //     let queue = self.inner.queue.clone();
        //     Context::new(backend, queue, request)
        // } else {
        //     return Signal::Skip;
        // };

        // TODO. wait until all done an repeat poll

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
