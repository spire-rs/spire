//! The [`Context`] a worker handles.

use std::cell::Cell;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use ::http::StatusCode;
use bytes::Bytes;
use tokio::sync::OnceCell;
use tokio::time::Instant;

use crate::Result;
use crate::backend::Session;
use crate::dataset::{Data, Dataset, Datasets};
use crate::frontier::InMemFrontier;
use crate::frontier::queue::Queue;
use crate::http::{Request, Response, ResponseExt};

/// A request being crawled, with everything needed to handle it.
///
/// It holds the [`Request`], a session to fetch it with, a [`Queue`] to add
/// the links it leads to, and the crawl's datasets. Handlers borrow it, so
/// several of them can handle one request: the request is fetched once, by
/// the first [`response`](Self::response) or [`body`](Self::body), and the
/// complete response is kept. Its body is a fixed snapshot, including for
/// browser sessions; live document operations go through the session.
///
/// The crawler learns how the fetch went, its time and status, from the task
/// the request runs on. Fetching after moving the context to another task,
/// e.g. with `tokio::spawn`, works, but adaptive host limits and
/// [`Stats::unchanged`](crate::crawler::Stats::unchanged) do not see it.
pub struct Context<C> {
    request: Request,
    session: C,
    queue: Queue,
    datasets: Arc<Datasets>,
    response: OnceCell<Response>,
}

/// How a request's fetch went: how long it took, and the response's status.
///
/// The crawler runs each request on a task of its own, with a [`FETCH`]
/// scope the context records the fetch into, so the crawler reads it back
/// without allocating or locking per request.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Fetch {
    pub(crate) latency: Option<Duration>,
    pub(crate) status: Option<StatusCode>,
}

tokio::task_local! {
    /// How the current task's request was fetched, see [`Fetch`].
    pub(crate) static FETCH: Cell<Fetch>;
}

impl<C> Context<C> {
    /// Creates a standalone context, e.g. to test a handler.
    ///
    /// It gets an empty queue of its own and no datasets; see
    /// [`with_queue`](Self::with_queue) and
    /// [`with_dataset`](Self::with_dataset).
    #[must_use]
    pub fn new(request: Request, session: C) -> Self {
        let queue = Queue::new(InMemFrontier::fifo()).child_of(&request);
        Self::from_parts(request, session, queue, Arc::default())
    }

    pub(crate) fn from_parts(
        request: Request,
        session: C,
        queue: Queue,
        datasets: Arc<Datasets>,
    ) -> Self {
        Self {
            request,
            session,
            queue,
            datasets,
            response: OnceCell::new(),
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

    /// Returns the session, including any live document capability.
    #[must_use]
    pub const fn session(&self) -> &C {
        &self.session
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

    /// Returns the complete response, fetching the request on the first call.
    ///
    /// Later calls return the same response without fetching again. A failed
    /// fetch is not kept: the next call tries again. Once fetched, links
    /// followed through the queue, including clones taken earlier, resolve
    /// against the URI the response was served from.
    ///
    /// # Errors
    ///
    /// Returns an error if the session fails to fetch the request.
    pub async fn response(&self) -> Result<&Response>
    where
        C: Session,
    {
        self.response
            .get_or_try_init(|| async {
                let started = Instant::now();
                let response = self.session.fetch(&self.request).await;
                // Outside a crawl, e.g. in a test, there is no scope to
                // record into.
                let fetch = Fetch {
                    latency: Some(started.elapsed()),
                    status: response.as_ref().ok().map(::http::Response::status),
                };
                let _ = FETCH.try_with(|cell| cell.set(fetch));
                let response = response?;
                if let Some(uri) = response.final_uri() {
                    self.queue.rebase(uri.clone());
                }
                Ok(response)
            })
            .await
    }

    /// Returns a cheap clone of the cached response body, fetching first
    /// if needed. Browser sessions also return a fixed HTML snapshot;
    /// use the session's document capability for live observations.
    ///
    /// # Errors
    ///
    /// Returns an error if the request cannot be fetched.
    pub async fn body(&self) -> Result<Bytes>
    where
        C: Session,
    {
        Ok(self.response().await?.body().clone())
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
    use crate::crawler::Depth;
    use crate::dataset::InMemDataset;
    use crate::frontier::seed::Seed;
    use crate::http::RequestExt;

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

    impl Session for Counting {
        fn fetch(&self, _request: &Request) -> impl Future<Output = Result<Response>> + Send {
            let fetch = self.fetches.fetch_add(1, Ordering::SeqCst);
            std::future::ready(if fetch < self.fail {
                Err(crate::Error::new(crate::ErrorKind::Http))
            } else {
                Ok(Response::new(Bytes::from_static(
                    self.body.lock().unwrap().as_bytes(),
                )))
            })
        }
    }

    #[tokio::test]
    async fn fetches_once_and_keeps_a_snapshot_of_the_body() {
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
        *cx.session().body.lock().unwrap() = "clicked";
        assert_eq!(cx.body().await.unwrap(), "loaded");
        assert_eq!(cx.response().await.unwrap().body(), "loaded");
        assert!(cx.response().await.unwrap().status().is_success());
        assert_eq!(cx.session().fetches.load(Ordering::SeqCst), 2);
    }
}
