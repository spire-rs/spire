use std::fmt;
use std::sync::Arc;

use crate::backend::{Backend, Worker};
use crate::context::Request;
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
    pub async fn run(self) -> Result<usize>
    where
        B: Backend,
        W: Worker<B>,
    {
        // TODO: Add tracing.
        // TODO: Add run for a single request.
        self.inner.run_until_empty().await
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
    pub fn with_request_queue<D, E>(self, dataset: D) -> Self
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
    use crate::backend::Worker;
    use crate::context::{Context, Signal};

    #[derive(Debug, Clone)]
    struct T;

    #[async_trait::async_trait]
    impl<B> Worker<B> for T
    where
        B: Send + 'static,
    {
        async fn invoke(self, _cx: Context<B>) -> Signal {
            Signal::default()
        }
    }

    #[test]
    #[cfg(feature = "client")]
    fn with_client() {
        let backend = crate::backend::HttpClient::default();
        let _ = crate::Daemon::new(backend, T);
    }

    #[test]
    #[cfg(feature = "driver")]
    fn with_driver() {
        let backend = crate::backend::BrowserPool::default();
        let _ = crate::Daemon::new(backend, T);
    }
}
