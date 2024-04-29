use std::fmt;
use std::sync::Arc;

use crate::backend::{Backend, Worker};
use crate::context::{Body, Request};
use crate::dataset::{util::BoxCloneDataset, Dataset};
use crate::process::runner::Runner;
use crate::{BoxError, Error, Result};

mod runner;

/// Orchestrates the processing of [`Request`]s using provided [`Backend`] and [`Worker`].
pub struct Client<B, W> {
    inner: Arc<Runner<B, W>>,
}

impl<B, W> Client<B, W> {
    /// Creates a new [`Client`] with provided [`Backend`] and [`Worker`].
    pub fn new(backend: B, inner: W) -> Self
    where
        B: Backend,
        W: Worker<B>,
    {
        let inner = Arc::new(Runner::new(backend, inner));
        Self { inner }
    }

    /// Processes [`Request`]s with a provided [`Worker`] until the [`RequestQueue`] is empty.
    ///
    /// [`RequestQueue`]: crate::context::RequestQueue
    pub async fn run(&self) -> Result<usize>
    where
        B: Backend,
        W: Worker<B>,
    {
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
    pub async fn run_once(&self, request: Request) -> Result<()>
    where
        B: Backend,
        W: Worker<B>,
    {
        self.inner.call_service(request).await;
        Ok(())
    }

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
        E: Into<BoxError>,
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
        E: Into<BoxError>,
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
    pub fn dataset<T>(&self) -> BoxCloneDataset<T, Error>
    where
        T: Send + Sync + 'static,
    {
        self.inner.datasets.get::<T>()
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

#[cfg(test)]
mod test {
    use http::Request;
    use tracing_test::traced_test;

    use crate::backend::util::TraceEntity;
    use crate::dataset::InMemDataset;
    use crate::{Client, Result};

    #[tokio::test]
    #[cfg_attr(feature = "tracing", traced_test)]
    async fn with_entity() -> Result<()> {
        let entity = TraceEntity::default();
        let request = Request::get("https://example.com/").body(());

        let client = Client::new(entity.clone(), entity)
            .with_request_queue(InMemDataset::stack())
            .with_dataset(InMemDataset::<u64>::new())
            .with_initial_request(request.unwrap());

        let _ = client.dataset::<u64>();
        let _ = client.run().await?;
        Ok(())
    }

    #[tokio::test]
    #[cfg_attr(feature = "tracing", traced_test)]
    #[cfg(feature = "client")]
    async fn with_client() -> Result<()> {
        use crate::backend::HttpClient;

        let backend = TraceEntity::new(HttpClient::default());
        let request = Request::get("https://example.com/").body(());

        let client = Client::new(backend, TraceEntity::default())
            .with_request_queue(InMemDataset::stack())
            .with_dataset(InMemDataset::<u64>::new())
            .with_initial_request(request.unwrap());

        let _ = client.dataset::<u64>();
        let _ = client.run().await?;
        Ok(())
    }

    #[tokio::test]
    #[cfg_attr(feature = "tracing", traced_test)]
    #[cfg(feature = "driver")]
    async fn with_driver() -> Result<()> {
        use crate::backend::BrowserPool;

        let backend = TraceEntity::new(BrowserPool::default());
        let request = Request::get("https://example.com/").body(());

        let client = Client::new(backend, TraceEntity::default())
            .with_request_queue(InMemDataset::stack())
            .with_dataset(InMemDataset::<u64>::new())
            .with_initial_request(request.unwrap());

        let _ = client.dataset::<u64>();
        let _ = client.run().await?;
        Ok(())
    }
}
