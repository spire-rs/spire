//! The [`Context`] a worker handles.

use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use tokio::sync::OnceCell;
use tokio::time::Instant;

use crate::Result;
use crate::backend::Client;
use crate::dataset::{Data, Dataset, Datasets};
use crate::frontier::memory::InMemFrontier;
use crate::frontier::queue::Queue;
use crate::request::{Request, Response, ResponseExt};
use crate::sync::lock;

/// A request being crawled, with everything needed to handle it.
///
/// It holds the [`Request`], a client to fetch it with, a [`Queue`] to add
/// the links it leads to, and the crawl's datasets. Handlers borrow it, so
/// several of them can handle one request: the request is fetched once, by
/// the first [`response`](Self::response) or [`body`](Self::body), and the
/// response's head is kept. The body is always read from the client, so a
/// browser page reads as currently rendered.
pub struct Context<C> {
    request: Request,
    client: C,
    queue: Queue,
    datasets: Arc<Datasets>,
    response: OnceCell<Response<()>>,
    /// Where to record how long the fetch took, for adaptive host delays.
    fetch_time: Option<FetchTime>,
}

/// How long a request's fetch took, shared with the crawler.
pub(crate) type FetchTime = Arc<Mutex<Option<Duration>>>;

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

    pub(crate) fn from_parts(
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
            response: OnceCell::new(),
            fetch_time: None,
        }
    }

    /// Records how long the fetch takes in `fetch_time`.
    pub(crate) fn with_fetch_time(mut self, fetch_time: FetchTime) -> Self {
        self.fetch_time = Some(fetch_time);
        self
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

    /// Returns the response's head, fetching the request on the first call.
    ///
    /// Later calls return the same head without fetching again. A failed
    /// fetch is not kept: the next call tries again. Once fetched, links
    /// followed through the queue, including clones taken earlier, resolve
    /// against the URI the response was served from.
    ///
    /// # Errors
    ///
    /// Returns an error if the client fails to fetch the request.
    pub async fn response(&self) -> Result<&Response<()>>
    where
        C: Client,
    {
        self.response
            .get_or_try_init(|| async {
                let started = Instant::now();
                let response = self.client.fetch(&self.request).await;
                if let Some(fetch_time) = &self.fetch_time {
                    *lock(fetch_time) = Some(started.elapsed());
                }
                let response = response?;
                if let Some(uri) = response.final_uri() {
                    self.queue.rebase(uri.clone());
                }
                Ok(response)
            })
            .await
    }

    /// Returns the response's body, fetching the request first if needed:
    /// the bytes received, or a browser page as currently rendered.
    ///
    /// # Errors
    ///
    /// Returns an error if the request cannot be fetched or its body read.
    pub async fn body(&self) -> Result<Bytes>
    where
        C: Client,
    {
        self.response().await?;
        self.client.body().await
    }
}

impl<C> fmt::Debug for Context<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context")
            .field("method", self.request.method())
            .field("uri", self.request.uri())
            .field("queue", &self.queue)
            .field("fetched", &self.response.initialized())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::backend::Noop;
    use crate::dataset::InMemDataset;
    use crate::frontier::seed::Seed;
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

        let response = cx.response().await.unwrap();
        assert!(response.status().is_success());
    }

    /// Counts its fetches, failing the first `fail` of them, and serves a
    /// body that changes after the fetch, as a browser page can.
    struct Counting {
        fetches: AtomicUsize,
        fail: usize,
        body: std::sync::Mutex<&'static str>,
    }

    impl Client for Counting {
        fn fetch(&self, _request: &Request) -> impl Future<Output = Result<Response<()>>> + Send {
            let fetch = self.fetches.fetch_add(1, Ordering::SeqCst);
            std::future::ready(if fetch < self.fail {
                Err(crate::Error::new(crate::ErrorKind::Http))
            } else {
                Ok(Response::new(()))
            })
        }

        fn body(&self) -> impl Future<Output = Result<Bytes>> + Send {
            let body = *self.body.lock().unwrap();
            std::future::ready(Ok(Bytes::from_static(body.as_bytes())))
        }
    }

    #[tokio::test]
    async fn fetches_once_and_reads_the_current_body() {
        let request = Seed::get("https://example.com/").unwrap().into_request();
        let client = Counting {
            fetches: AtomicUsize::new(0),
            fail: 1,
            body: std::sync::Mutex::new("loaded"),
        };
        let cx = Context::new(request, client);

        // A failed fetch is tried again; a successful one is kept.
        assert!(cx.response().await.is_err());
        assert_eq!(cx.body().await.unwrap(), "loaded");
        *cx.client().body.lock().unwrap() = "clicked";
        assert_eq!(cx.body().await.unwrap(), "clicked");
        assert!(cx.response().await.unwrap().status().is_success());
        assert_eq!(cx.client().fetches.load(Ordering::SeqCst), 2);
    }
}
