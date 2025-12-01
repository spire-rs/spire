//! Internal runner for executing web scraping tasks.
//!
//! This module contains the [`Runner`] type which handles the low-level execution
//! of requests, signal processing, and coordination between the backend and worker.

use std::cmp::max;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use futures::StreamExt;
use futures::stream::{AbortHandle, Abortable};
use tokio_util::sync::CancellationToken;

use crate::Result;
#[cfg(feature = "tracing")]
use crate::TRACING_TARGET_RUNNER as TARGET;
use crate::backend::{Backend, Worker};
use crate::context::{Context, IntoSignal, Request, Signal, Tag, TagQuery, Task};
use crate::dataset::{Dataset, DatasetExt, DatasetRegistry};

/// Internal runner that executes web scraping tasks.
///
/// The `Runner` is responsible for:
/// - Managing the request queue and datasets
/// - Coordinating between backend and worker
/// - Processing signals (wait, hold, fail, etc.)
/// - Handling deferred and aborted requests
/// - Graceful shutdown via cancellation token
pub struct Runner<B, W> {
    service: W,
    pub datasets: DatasetRegistry,
    backend: B,

    pub initial: Mutex<Vec<Request>>,
    pub limit: AtomicUsize,
    // Fallback means all not-yet encountered tags.
    defer: Mutex<HashMap<Tag, Instant>>,
    /// Cancellation token for graceful shutdown
    shutdown: CancellationToken,
}

impl<B, W> Runner<B, W> {
    /// Creates a new [`Runner`].
    pub fn new(backend: B, inner: W) -> Self
    where
        B: Backend,
        W: Worker<B::Client>,
    {
        Self {
            service: inner,
            datasets: DatasetRegistry::new(),
            backend,

            initial: Mutex::new(Vec::new()),
            limit: AtomicUsize::new(8),
            defer: Mutex::new(HashMap::new()),
            shutdown: CancellationToken::new(),
        }
    }

    /// Returns a clone of the shutdown token.
    ///
    /// This token can be used to trigger graceful shutdown of the runner from outside.
    /// When cancelled, the runner will stop processing new requests and finish current ones.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use tokio::time::{sleep, Duration};
    ///
    /// let client = Client::new(backend, worker);
    /// let shutdown = client.shutdown_token();
    ///
    /// // Spawn runner in background
    /// tokio::spawn(async move {
    ///     client.run().await
    /// });
    ///
    /// // Trigger shutdown after 5 seconds
    /// sleep(Duration::from_secs(5)).await;
    /// shutdown.cancel();
    /// ```
    pub fn shutdown_token(&self) -> CancellationToken {
        self.shutdown.clone()
    }

    /// Repeatedly calls the used [`Backend`] until the [`Request`] queue is empty
    /// or the stream is aborted with a [`Signal`].
    ///
    /// Returns the total amount of processed `Request`s.
    ///
    /// ## Note
    ///
    /// Initial requests are consumed when this method is called. If an error occurs
    /// during processing, unconsumed requests in the dataset may be lost.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(skip(self), fields(concurrent_limit))
    )]
    pub async fn run(&self) -> Result<usize>
    where
        B: Backend,
        W: Worker<B::Client>,
    {
        #[cfg(feature = "tracing")]
        let start = Instant::now();
        let mut requests: Vec<_> = {
            let mut initial = self
                .initial
                .lock()
                .expect("Runner initial requests mutex poisoned");
            initial.drain(..).collect()
        };

        let dataset = self.datasets.get::<Request>();
        while let Some(request) = requests.pop() {
            dataset.write(request).await?;
        }

        let concurrent_limit = self.limit.load(Ordering::SeqCst);

        #[cfg(feature = "tracing")]
        tracing::Span::current().record("concurrent_limit", concurrent_limit);

        #[cfg(feature = "tracing")]
        tracing::info!(target: TARGET, initial_requests = requests.len(), "starting runner");

        let (handle, registration) = AbortHandle::new_pair();
        let stream = Abortable::new(dataset.into_stream(), registration);

        // Clone shutdown token for monitoring
        let shutdown = self.shutdown.clone();

        // Spawn shutdown monitor task
        let shutdown_handle = handle.clone();
        tokio::spawn(async move {
            shutdown.cancelled().await;
            #[cfg(feature = "tracing")]
            tracing::info!(target: TARGET, "shutdown requested, stopping request processing");
            shutdown_handle.abort();
        });

        let stream = stream
            // Abort on request queue/stream failures.
            .filter_map(|x| async { x.inspect_err(|_| handle.abort()).ok() })
            // Invoke the  underlying backend/worker.
            .then(|x| async move { self.run_once(x).await })
            // Abort on underlying backend/worker failures.
            .map(|x| async { x.inspect_err(|_| handle.abort()) })
            // Other.
            .buffer_unordered(concurrent_limit)
            .count();

        let total = stream.await;

        #[cfg(feature = "tracing")]
        if self.shutdown.is_cancelled() {
            tracing::info!(target: TARGET, "runner stopped due to shutdown request");
        }

        #[cfg(feature = "tracing")]
        {
            let duration = start.elapsed();
            tracing::info!(
                target: TARGET,
                total_requests = total,
                duration_ms = duration.as_millis(),
                requests_per_sec = (total as f64 / duration.as_secs_f64()) as u64,
                "runner completed"
            );
        }

        Ok(total)
    }

    /// Calls the used [`Backend`] once with a provided [`Request`].
    ///
    /// # Errors
    ///
    /// Only if the `Request` stream should be aborted.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self, req), fields(
        uri = %req.uri(),
        method = %req.method(),
        depth = req.depth()
    )))]
    pub async fn run_once(&self, req: Request) -> Result<()>
    where
        B: Backend,
        W: Worker<B::Client>,
    {
        #[cfg(feature = "tracing")]
        tracing::debug!(target: TARGET, "processing request");

        match self.call_service(req).await {
            Ok((signal, owner)) => {
                #[cfg(feature = "tracing")]
                tracing::debug!(
                    target: TARGET,
                    signal = ?signal,
                    owner = ?owner,
                    "request completed"
                );
                self.notify(signal, owner)
            }
            Err(x) => {
                #[cfg(feature = "tracing")]
                tracing::warn!(
                    target: TARGET,
                    error = %x,
                    "request failed"
                );
                self.notify(x.into_signal(), Tag::Fallback)
            }
        }
    }

    /// Creates the [`Context`] and calls the used [`Backend`] with it.
    ///
    /// Returns [`Signal`] and owner [`Tag`].
    ///
    /// ## Note
    ///
    /// Deferred request handling is not yet implemented and will be added in a future version.
    async fn call_service(&self, request: Request) -> Result<(Signal, Tag)>
    where
        B: Backend,
        W: Worker<B::Client>,
    {
        let owner = request.tag().clone();
        let datasets = self.datasets.clone();

        #[cfg(feature = "tracing")]
        tracing::trace!(target: TARGET, "acquiring backend client");

        let client: B::Client = self.backend.client().await?;

        #[cfg(feature = "tracing")]
        tracing::trace!(target: TARGET, "invoking worker");

        let cx: Context<B::Client> = Context::new(request, client, datasets);
        let signal = self.service.clone().invoke(cx).await;

        Ok((signal, owner))
    }

    /// Applies the [`Signal`] to the subsequent [`Request`]s.
    ///
    /// # Errors
    ///
    /// Only if the `Request` stream should be aborted.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self, signal), fields(owner = ?owner)))]
    pub fn notify(&self, signal: Signal, owner: Tag) -> Result<()> {
        #[cfg(feature = "tracing")]
        match &signal {
            Signal::Wait(query, duration) => {
                tracing::debug!(target: TARGET, query = ?query, duration_ms = duration.as_millis(), "deferring tags");
            }
            Signal::Hold(query, duration) => {
                tracing::debug!(target: TARGET, query = ?query, duration_ms = duration.as_millis(), "holding tags");
            }
            Signal::Fail(query, _) => {
                tracing::warn!(target: TARGET, query = ?query, "aborting tags");
            }
            Signal::Continue => {
                tracing::trace!(target: TARGET, "continuing");
            }
            Signal::Skip => {
                tracing::trace!(target: TARGET, "skipping");
            }
        }

        match signal {
            Signal::Wait(x, t) | Signal::Hold(x, t) => self.apply_defer(x, owner, t),
            Signal::Fail(x, _) => self.apply_abort(x, owner)?,
            Signal::Continue | Signal::Skip => {}
        }

        Ok(())
    }

    /// Defers all [`Tag`]s as specified per [`TagQuery`].
    ///
    /// Marks tags to be delayed for the specified duration. Deferred tags will not
    /// be processed until the delay expires.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self), fields(query = ?query, owner = ?owner, duration_ms = duration.as_millis()), level = "trace"))]
    fn apply_defer(&self, query: TagQuery, owner: Tag, duration: Duration) {
        let minimum = Instant::now() + duration;
        let mut defer = self.defer.lock().expect("Runner defer mutex poisoned");

        let mut defer_one = |x: Tag| {
            let _ = match defer.entry(x) {
                Entry::Occupied(mut x) => x.insert(max(*x.get() + duration, minimum)),
                Entry::Vacant(x) => *x.insert(minimum),
            };
        };

        match query {
            TagQuery::Owner => defer_one(owner),
            TagQuery::Single(x) => defer_one(x),
            TagQuery::Every => defer_one(Tag::Fallback),
            TagQuery::List(x) => x.into_iter().for_each(defer_one),
        };
    }

    /// Aborts all [`Tag`]s as specified per [`TagQuery`].
    ///
    /// ## Note
    ///
    /// This functionality is not yet implemented. Currently this method does nothing
    /// and always returns `Ok(())`. Full abort functionality will be added in a future version.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self), fields(query = ?query, owner = ?owner), level = "trace"))]
    fn apply_abort(&self, query: TagQuery, owner: Tag) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::warn!(target: TARGET, "abort functionality not yet implemented");

        let _ = query;
        let _ = owner;

        Ok(())
    }
}
