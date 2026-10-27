use std::fmt;
use std::sync::Arc;

use super::MockSite;
use crate::backend::Backend;
use crate::dataset::{Data, Dataset, Datasets};
use crate::frontier::queue::Queue;
use crate::frontier::seed::Seed;
use crate::frontier::{Frontier, InMemFrontier};
use crate::worker::{Outcome, Worker};
use crate::{Context, Result};

/// One request to a [`MockSite`], run through a worker without crawling the
/// links it queues.
///
/// Created by [`MockSite::visit`]. After [`run`](Self::run), the worker's
/// effects can be checked: the [`Outcome`] it returned, the seeds it
/// [`queued`](Self::queued), and the items it wrote to its datasets.
///
/// The queue has no facades: no deduplication, filters or depth limits, so
/// every link the worker follows is kept.
pub struct Visit {
    site: MockSite,
    seed: Seed,
    frontier: Arc<InMemFrontier>,
    datasets: Arc<Datasets>,
}

impl Visit {
    pub(super) fn new(site: MockSite, seed: Seed) -> Self {
        Self {
            site,
            seed,
            frontier: Arc::new(InMemFrontier::fifo()),
            datasets: Arc::default(),
        }
    }

    /// Registers `dataset` for items of type `T`.
    #[must_use]
    pub fn with_dataset<T, D>(mut self, dataset: D) -> Self
    where
        T: Send + 'static,
        D: Dataset<T>,
    {
        Arc::make_mut(&mut self.datasets).insert(dataset);
        self
    }

    /// Fetches the request and runs `worker` on it, returning its outcome.
    ///
    /// Each run is a new request: running twice fetches twice, and adds to
    /// the same queue and datasets.
    pub async fn run<W: Worker<<MockSite as Backend>::Session>>(&self, worker: &W) -> Outcome {
        let client = match self.site.connect().await {
            Ok(client) => client,
            Err(error) => return Outcome::Fail(error),
        };
        let request = self.seed.clone().into_request();
        let queue = Queue::new(Arc::clone(&self.frontier)).child_of(&request);
        let cx = Context::from_parts(request, client, queue, Arc::clone(&self.datasets));
        worker.handle(cx).await
    }

    /// Returns the seeds queued so far, in order.
    #[must_use]
    pub fn queued(&self) -> Vec<Seed> {
        self.frontier.snapshot().unwrap_or_default()
    }

    /// Returns the dataset for items of type `T`.
    ///
    /// # Errors
    ///
    /// Returns a [`Dataset`](crate::ErrorKind::Dataset) error if no dataset
    /// is registered for `T`.
    pub fn dataset<T: Send + 'static>(&self) -> Result<Data<T>> {
        self.datasets.get()
    }
}

impl fmt::Debug for Visit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Visit")
            .field("seed", &self.seed)
            .field("queued", &self.frontier.size_hint())
            .finish_non_exhaustive()
    }
}
