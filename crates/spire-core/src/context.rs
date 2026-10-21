//! The [`Context`] a worker handles.

use std::fmt;
use std::sync::Arc;

use crate::Result;
use crate::backend::Client;
use crate::dataset::{Data, Dataset, Datasets};
use crate::frontier::{InMemFrontier, Queue};
use crate::request::{Request, Response, ResponseExt};

/// A request being crawled, with everything needed to handle it.
///
/// It holds the [`Request`], a client to fetch it with, a [`Queue`] to add
/// the links it leads to, and the crawl's datasets.
pub struct Context<C> {
    request: Request,
    client: C,
    queue: Queue,
    datasets: Arc<Datasets>,
}

impl<C> Context<C> {
    /// Creates a standalone context, e.g. to test a handler.
    ///
    /// It gets an empty queue of its own and no datasets; see
    /// [`with_queue`](Self::with_queue) and
    /// [`with_dataset`](Self::with_dataset).
    #[must_use]
    pub fn new(request: Request, client: C) -> Self {
        let queue = Queue::new(InMemFrontier::fifo()).child_of(&request);
        Self::from_parts(request, client, queue, Arc::default())
    }

    pub(crate) const fn from_parts(
        request: Request,
        client: C,
        queue: Queue,
        datasets: Arc<Datasets>,
    ) -> Self {
        Self {
            request,
            client,
            queue,
            datasets,
        }
    }

    /// Replaces the queue, binding it to the request.
    #[must_use]
    pub fn with_queue(mut self, queue: &Queue) -> Self {
        self.queue = queue.child_of(&self.request);
        self
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

    /// Returns the request.
    #[must_use]
    pub const fn request(&self) -> &Request {
        &self.request
    }

    /// Returns the request mutably, e.g. to add headers before fetching.
    pub const fn request_mut(&mut self) -> &mut Request {
        &mut self.request
    }

    /// Returns the client.
    #[must_use]
    pub const fn client(&self) -> &C {
        &self.client
    }

    /// Returns the client mutably.
    pub const fn client_mut(&mut self) -> &mut C {
        &mut self.client
    }

    /// Returns the queue for the links this request leads to.
    #[must_use]
    pub const fn queue(&self) -> &Queue {
        &self.queue
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

    /// Splits the context into its request and client.
    #[must_use]
    pub fn into_parts(self) -> (Request, C) {
        (self.request, self.client)
    }

    /// Fetches the response for the request.
    ///
    /// Links followed through the queue, including clones taken earlier,
    /// resolve against the URI the response was served from from then on.
    ///
    /// # Errors
    ///
    /// Returns an error if the client fails to fetch the response.
    pub async fn fetch(self) -> Result<Response>
    where
        C: Client,
    {
        let response = self.client.fetch(self.request).await?;
        if let Some(uri) = response.final_uri() {
            self.queue.rebase(uri.clone());
        }
        Ok(response)
    }
}

impl<C> fmt::Debug for Context<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context")
            .field("method", self.request.method())
            .field("uri", self.request.uri())
            .field("queue", &self.queue)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Noop;
    use crate::dataset::InMemDataset;
    use crate::frontier::Seed;
    use crate::request::{Depth, RequestExt};

    #[tokio::test]
    async fn standalone_context() {
        let request = Seed::get("https://example.com/").unwrap().into_request();
        let cx = Context::new(request, Noop).with_dataset(InMemDataset::<u32>::new());

        cx.dataset::<u32>().unwrap().write(1).await.unwrap();
        assert!(cx.dataset::<String>().is_err());

        cx.queue().follow("https://example.com/next").await.unwrap();
        assert_eq!(cx.queue().size_hint(), Some(1));
        assert_eq!(cx.request().depth(), Depth::ROOT);

        let response = cx.fetch().await.unwrap();
        assert!(response.status().is_success());
    }
}
