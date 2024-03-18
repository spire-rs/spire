use std::fmt;
use std::future::Future;
use std::sync::Arc;

use futures::future::BoxFuture;

use runner::Runner;

use crate::backend::{Backend, Router};
use crate::context::Request;
use crate::dataset::util::BoxCloneDataset;
use crate::dataset::Dataset;
use crate::{BoxError, Error, Result};

mod metric;
mod runner;

/// TODO.
pub struct Daemon<B, S> {
    inner: Arc<Runner<B, S>>,
}

impl<B, S> Daemon<B, S> {
    /// Creates a new [`Daemon`].
    pub fn new(backend: B, inner: S) -> Self
    where
        B: Backend,
        S: Router<B>,
    {
        let inner = Arc::new(Runner::new(backend, inner));
        Self { inner }
    }

    pub async fn run(self) -> Result<usize>
    where
        B: Backend,
        S: Router<B>,
    {
        // TODO: Add tracing.
        self.inner.run_until_empty().await
    }

    // TODO: Replace run.
    pub async fn run2(self) -> Result<DaemonHandle>
    where
        B: Backend,
        S: Router<B>,
    {
        let fut = self.inner.run_until_empty();
        todo!()
    }

    /// Replaces the [`Dataset`] used by the [`RequestQueue`].
    ///
    /// If the `Dataset` for the `Queue` is not provided, then
    /// the queue backed by the [`InMemDataset`] is used instead.
    ///
    /// ### Note
    ///
    /// Does not move items from the replaced `Dataset`.
    ///
    /// [`InMemDataset`]: crate::dataset::InMemDataset
    /// [`RequestQueue`]: crate::context::RequestQueue
    pub fn with_queue<D, E>(self, dataset: D) -> Self
    where
        D: Dataset<Request, Error = E> + Clone,
        E: Into<BoxError>,
    {
        self.inner.datasets.set(dataset);
        self
    }

    /// Inserts the provided [`Dataset`].
    ///
    /// ### Note
    ///
    /// Replaces the dataset of the same type if it is already inserted.
    /// Does not move items from the replaced `Dataset`.
    ///
    /// If the handler requests for a [`Dataset`] of a specific type, but  no `Dataset` of this
    /// type was provided, it will be lazily initialized as a `first-in first-out` [`InMemDataset`].
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

/// TODO.
pub struct DaemonHandle {
    // TODO: tokio join handle?
    fut: BoxFuture<'static, Result<usize>>,
}

impl DaemonHandle {
    /// Creates a new [`DaemonHandle`].
    pub(crate) fn new<F>(fut: F) -> Self
    where
        F: Future<Output = Result<usize>>,
    {
        todo!()
    }

    /// Waits until the TODO.
    pub async fn wait(self) -> Result<usize> {
        todo!()
    }
}

#[cfg(test)]
mod test {
    #[cfg(feature = "driver")]
    use crate::backend::BrowserPool;
    #[cfg(feature = "client")]
    use crate::backend::HttpClient;
    use crate::{backend::Router, Daemon};

    fn make_service<B>() -> impl Router<B> {
        todo!()
    }

    #[test]
    #[cfg(feature = "client")]
    fn with_client() {
        let backend = HttpClient::default();
        let service = make_service();
        let _ = Daemon::new(backend, service);
    }

    #[test]
    #[cfg(feature = "driver")]
    fn with_driver() {
        let backend = BrowserPool::default();
        let service = make_service();
        let _ = Daemon::new(backend, service);
    }
}
