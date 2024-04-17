use std::fmt;
use std::sync::Arc;

use crate::backend::{Backend, Worker};
use crate::context::{Body, Request};
use crate::daemon::runner::Runner;
use crate::dataset::util::BoxCloneDataset;
use crate::dataset::Dataset;
use crate::{BoxError, Error, Result};

mod runner;

/// TODO.
/// is a polling loop
pub struct Daemon<B, W> {
    inner: Arc<Runner<B, W>>,
}

impl<B, W> Daemon<B, W> {
    /// Creates a new [`Daemon`].
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
    /// It's preferred to use [`Daemon::with_initial_request`] and [`Daemon::run`] instead.
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

    /// Adds a single [`Request`] to the [`RequestQueue`] when a [`Daemon::run`] is invoked.
    ///
    /// # Note
    ///
    /// See [`Daemon::with_initial_requests`] for multiple `Request`s.
    ///
    /// [`RequestQueue`]: crate::context::RequestQueue
    pub fn with_initial_request<R>(self, request: Request<R>) -> Self
    where
        R: Into<Body>,
    {
        self.inner.add_initial(request.map(Into::into));
        self
    }

    /// Adds a set of [`Request`]s to the [`RequestQueue`] when a [`Daemon::run`] is invoked.
    ///
    /// # Note
    ///
    /// See [`Daemon::with_initial_request`] for a single `Request`.
    ///
    /// [`RequestQueue`]: crate::context::RequestQueue
    pub fn with_initial_requests<T, R>(self, requests: T) -> Self
    where
        T: IntoIterator<Item = Request<R>>,
        R: Into<Body>,
    {
        requests.into_iter().for_each(|request| {
            self.inner.add_initial(request.map(Into::into));
        });

        self
    }

    /// Inserts the provided [`Dataset`].
    ///
    /// ### Note
    ///
    /// Replaces the dataset of the same type if it is already inserted.
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

    /// Returns the [`Dataset`] of the requested type.
    ///
    /// ### Note
    ///
    /// Inserts and returns the [`InMemDataset`] if none was found.
    ///
    /// [`InMemDataset`]: crate::dataset::InMemDataset
    pub fn dataset<T>(&self) -> BoxCloneDataset<T, Error>
    where
        T: Send + Sync + 'static,
    {
        self.inner.datasets.get::<T>()
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
        f.debug_struct("Daemon")
            .field("Datasets", &self.inner.datasets.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod test {
    use http::Request;

    use crate::backend::DebugEntity;
    use crate::dataset::InMemDataset;

    #[test]
    fn with_entity() {
        let entity = crate::backend::TraceEntity::default();
        let request = Request::get("https://example.com/").body(());

        let _ = crate::Daemon::new(entity, DebugEntity::new())
            .with_request_queue(InMemDataset::stack())
            .with_dataset(InMemDataset::<u64>::new())
            .with_initial_request(request.unwrap());
    }

    #[test]
    #[cfg(feature = "client")]
    fn with_client() {
        let backend = crate::backend::HttpClient::default();
        let request = Request::get("https://example.com/").body(());

        let _ = crate::Daemon::new(backend, DebugEntity::new())
            .with_request_queue(InMemDataset::stack())
            .with_dataset(InMemDataset::<u64>::new())
            .with_initial_request(request.unwrap());
    }

    #[test]
    #[cfg(feature = "driver")]
    fn with_driver() {
        let backend = crate::backend::BrowserPool::default();
        let request = Request::get("https://example.com/").body(());

        let _ = crate::Daemon::new(backend, DebugEntity::new())
            .with_request_queue(InMemDataset::stack())
            .with_dataset(InMemDataset::<u64>::new())
            .with_initial_request(request.unwrap());
    }
}
