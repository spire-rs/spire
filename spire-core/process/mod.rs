use std::fmt;
use std::sync::Arc;

use crate::backend::{Backend, Worker};
use crate::context::{Body, Request};
use crate::dataset::{Data, Dataset};
use crate::process::runner::Runner;
use crate::{Error, Result};

mod runner;

/// Orchestrates the processing of [`Request`]s using provided [`Backend`] and [`Worker`].
#[must_use]
pub struct Client<B, W> {
    inner: Arc<Runner<B, W>>,
}

impl<B, W> Client<B, W>
where
    B: Backend,
    W: Worker<B::Client>,
{
    /// Creates a new [`Client`] with provided [`Backend`] and [`Worker`].
    pub fn new(backend: B, inner: W) -> Self {
        let inner = Arc::new(Runner::new(backend, inner));
        Self { inner }
    }

    /// Processes [`Request`]s with a provided [`Worker`] until the [`RequestQueue`] is empty.
    ///
    /// [`RequestQueue`]: crate::context::RequestQueue
    pub async fn run(&self) -> Result<usize> {
        self.inner.run_until_empty().await
    }

    /// Processes a single provided [`Request`].
    ///
    /// ### Note
    ///
    /// It's preferred to use [`Client::with_initial_request`] and [`Client::run`] instead.
    ///
    /// Does not process the [`RequestQueue`].
    ///
    /// [`RequestQueue`]: crate::context::RequestQueue
    pub async fn run_once(&self, request: Request) -> Result<()> {
        self.inner.call_service(request).await?;
        Ok(())
    }
}

impl<B, W> Client<B, W> {
    /// Replaces the [`Dataset`] used by the [`RequestQueue`].
    ///
    /// If the `Dataset` for the `RequestQueue` is not provided, then
    /// the queue backed by the [`InMemDataset`] is used instead.
    ///
    /// ### Note
    ///
    /// Does not move items from the replaced `Dataset`.
    ///
    /// [`InMemDataset`]: crate::dataset::InMemDataset
    /// [`RequestQueue`]: crate::context::RequestQueue
    pub fn with_request_queue<D, E>(self, dataset: D) -> Self
    where
        D: Dataset<Request, Error = E> + Clone,
        Error: From<E>,
    {
        self.inner.datasets.set(dataset);
        self
    }

    /// Adds a single [`Request`] to the [`RequestQueue`] when a [`Client::run`] is invoked.
    ///
    /// # Note
    ///
    /// See [`Client::with_initial_requests`] for multiple `Request`s.
    ///
    /// [`RequestQueue`]: crate::context::RequestQueue
    pub fn with_initial_request<R>(self, request: Request<R>) -> Self
    where
        R: Into<Body>,
    {
        self.inner.with_initial_request(request.map(Into::into));
        self
    }

    /// Adds a set of [`Request`]s to the [`RequestQueue`] when a [`Client::run`] is invoked.
    ///
    /// # Note
    ///
    /// See [`Client::with_initial_request`] for a single `Request`.
    ///
    /// [`RequestQueue`]: crate::context::RequestQueue
    pub fn with_initial_requests<T, R>(self, requests: T) -> Self
    where
        T: IntoIterator<Item = Request<R>>,
        R: Into<Body>,
    {
        requests.into_iter().for_each(|request| {
            self.inner.with_initial_request(request.map(Into::into));
        });

        self
    }

    /// Limits a number of buffered (or in-flight) futures.
    pub fn with_concurrency_limit(self, limit: usize) -> Self {
        self.inner.with_concurrency_limit(limit);
        self
    }

    /// Inserts the provided [`Dataset`].
    ///
    /// ### Note
    ///
    /// Replaces the dataset for the same type if it is already inserted.
    /// Does not move items from the replaced `Dataset`.
    ///
    /// If the handler requests for a [`Dataset`] of a specific type, but  no `Dataset` of
    /// this type was provided, it will be lazily initialized as a `first-in first-out`
    /// [`InMemDataset`].
    ///
    /// [`InMemDataset`]: crate::dataset::InMemDataset
    pub fn with_dataset<D, E, T>(self, dataset: D) -> Self
    where
        D: Dataset<T, Error = E> + Clone,
        Error: From<E>,
        T: Send + Sync + 'static,
    {
        self.inner.datasets.set(dataset);
        self
    }

    /// Returns the [`Dataset`] for the requested type.
    ///
    /// ### Note
    ///
    /// Inserts and returns the [`InMemDataset`] if none were found.
    ///
    /// [`InMemDataset`]: crate::dataset::InMemDataset
    #[inline]
    #[must_use]
    pub fn dataset<T>(&self) -> Data<T>
    where
        T: Send + Sync + 'static,
    {
        Data::new(self.inner.datasets.get::<T>())
    }
}

impl<B, S> Clone for Client<B, S> {
    #[inline]
    fn clone(&self) -> Self {
        let inner = self.inner.clone();
        Self { inner }
    }
}

impl<B, S> fmt::Debug for Client<B, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Daemon")
            .field("Datasets", &self.inner.datasets.len())
            .finish_non_exhaustive()
    }
}
