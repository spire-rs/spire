//! Client for processing web scraping requests.
//!
//! This module provides the [`Client`] type, which orchestrates the execution
//! of web scraping tasks using a backend and worker implementation.

use std::cmp::max;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::backend::{Backend, Worker};
use crate::context::{Body, Request};
use crate::dataset::{Data, Dataset};
use crate::process::Runner;
use crate::{Error, Result};

/// Orchestrates the processing of [`Request`]s using provided [`Backend`] and [`Worker`].
///
/// The `Client` is the main entry point for running web scraping tasks. It manages:
/// - Request queue processing
/// - Dataset storage for scraped data
/// - Concurrency control
/// - Signal-based flow control
///
/// # Type Parameters
///
/// - `B`: The [`Backend`] implementation (e.g., HTTP client)
/// - `W`: The [`Worker`] implementation that processes requests
///
/// # Examples
///
/// ```ignore
/// use spire_core::Client;
///
/// let client = Client::new(backend, worker)
///     .with_concurrency_limit(10)
///     .with_initial_request(request);
///
/// let processed = client.run().await?;
/// println!("Processed {} requests", processed);
/// ```
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
    ///
    /// # Parameters
    ///
    /// - `backend`: The backend implementation for making HTTP requests
    /// - `inner`: The worker implementation for processing requests
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_core::Client;
    ///
    /// let client = Client::new(my_backend, my_worker);
    /// ```
    pub fn new(backend: B, inner: W) -> Self {
        let inner = Arc::new(Runner::new(backend, inner));
        Self { inner }
    }

    /// Processes [`Request`]s with a provided [`Worker`] until the [`RequestQueue`] is empty
    /// or the processing is aborted with a [`Signal`].
    ///
    /// This is the main execution method that processes all queued requests concurrently
    /// according to the configured concurrency limit.
    ///
    /// # Returns
    ///
    /// Returns the total number of successfully processed requests.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The backend fails to create a client
    /// - A request fails with a [`Signal::Fail`]
    /// - The request queue encounters an error
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_core::Client;
    ///
    /// let client = Client::new(backend, worker)
    ///     .with_initial_request(request);
    ///
    /// let count = client.run().await?;
    /// println!("Processed {} requests", count);
    /// ```
    ///
    /// [`RequestQueue`]: crate::context::RequestQueue
    /// [`Signal`]: crate::context::Signal
    /// [`Signal::Fail`]: crate::context::Signal::Fail
    pub async fn run(&self) -> Result<usize> {
        self.inner.run().await
    }

    /// Processes a single provided [`Request`].
    ///
    /// This method processes a single request immediately without using the request queue.
    /// It's useful for one-off requests or testing, but for production scraping workflows,
    /// it's better to use [`Client::with_initial_request`] and [`Client::run`].
    ///
    /// ## Note
    ///
    /// This method does not process the [`RequestQueue`]. Any requests added to the queue
    /// by the worker during processing will remain in the queue.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend or worker fails during request processing.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_core::Client;
    ///
    /// let client = Client::new(backend, worker);
    /// client.run_once(single_request).await?;
    /// ```
    ///
    /// [`RequestQueue`]: crate::context::RequestQueue
    pub async fn run_once(&self, req: Request) -> Result<()> {
        self.inner.run_once(req).await
    }
}

impl<B, W> Client<B, W> {
    /// Replaces the [`Dataset`] used by the [`RequestQueue`].
    ///
    /// By default, the request queue uses an in-memory FIFO dataset. This method
    /// allows you to provide a custom dataset implementation, such as a persistent
    /// queue backed by a database or file system.
    ///
    /// ## Note
    ///
    /// This method does not migrate items from the default dataset to the new one.
    /// Any requests already in the queue will be lost.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_core::Client;
    /// use spire_core::dataset::InMemDataset;
    ///
    /// let client = Client::new(backend, worker)
    ///     .with_request_queue(InMemDataset::stack()); // Use LIFO instead of FIFO
    /// ```
    ///
    /// [`InMemDataset`]: crate::dataset::InMemDataset
    /// [`RequestQueue`]: crate::context::RequestQueue
    pub fn with_request_queue<D, E>(self, dataset: D) -> Self
    where
        D: Dataset<Request, Error = E> + Clone + 'static,
        Error: From<E>,
    {
        self.inner.datasets.set(dataset);
        self
    }

    /// Adds a single [`Request`] to the [`RequestQueue`] when [`Client::run`] is called.
    ///
    /// Initial requests are queued immediately before processing begins. This is the
    /// primary way to seed the scraping pipeline with starting URLs.
    ///
    /// ## Note
    ///
    /// See [`Client::with_initial_requests`] for adding multiple requests at once.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use http::Request;
    /// use spire_core::Client;
    ///
    /// let request = Request::builder()
    ///     .uri("https://example.com")
    ///     .body(())?;
    ///
    /// let client = Client::new(backend, worker)
    ///     .with_initial_request(request);
    /// ```
    ///
    /// [`RequestQueue`]: crate::context::RequestQueue
    pub fn with_initial_request<R>(self, request: Request<R>) -> Self
    where
        R: Into<Body>,
    {
        let request = request.map(Into::into);
        self.inner
            .initial
            .lock()
            .expect("Client initial requests mutex poisoned")
            .push(request);
        self
    }

    /// Adds a set of [`Request`]s to the [`RequestQueue`] when [`Client::run`] is called.
    ///
    /// This method is convenient for seeding the scraping pipeline with multiple starting
    /// URLs at once, such as from a list or iterator.
    ///
    /// ## Note
    ///
    /// See [`Client::with_initial_request`] for adding a single request.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use http::Request;
    /// use spire_core::Client;
    ///
    /// let urls = vec!["https://example.com/1", "https://example.com/2"];
    /// let requests: Vec<_> = urls.into_iter()
    ///     .map(|url| Request::builder().uri(url).body(()).unwrap())
    ///     .collect();
    ///
    /// let client = Client::new(backend, worker)
    ///     .with_initial_requests(requests);
    /// ```
    ///
    /// [`RequestQueue`]: crate::context::RequestQueue
    pub fn with_initial_requests<T, R>(self, requests: T) -> Self
    where
        T: IntoIterator<Item = Request<R>>,
        R: Into<Body>,
    {
        let mut requests: Vec<_> = requests
            .into_iter()
            .map(|request| request.map(Into::into))
            .collect();

        self.inner
            .initial
            .lock()
            .expect("Client initial requests mutex poisoned")
            .append(&mut requests);
        self
    }

    /// Sets the maximum number of concurrent requests that can be processed simultaneously.
    ///
    /// This controls how many requests are processed in parallel. A higher limit increases
    /// throughput but also increases resource usage (memory, network connections, etc.).
    ///
    /// The default concurrency limit is 8. The minimum value is 1.
    ///
    /// # Parameters
    ///
    /// - `limit`: Maximum number of concurrent requests (minimum 1)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_core::Client;
    ///
    /// let client = Client::new(backend, worker)
    ///     .with_concurrency_limit(20); // Process up to 20 requests concurrently
    /// ```
    ///
    /// [`Future`]: std::future::Future
    pub fn with_concurrency_limit(self, limit: usize) -> Self {
        self.inner.limit.store(max(limit, 1), Ordering::SeqCst);
        self
    }

    /// Registers a [`Dataset`] for storing scraped data of type `T`.
    ///
    /// Datasets are used to store scraped data during the scraping process. Workers can
    /// access datasets through the context to write extracted data.
    ///
    /// ## Behavior
    ///
    /// - If a dataset for type `T` already exists, it will be replaced
    /// - Existing items in the replaced dataset are **not** migrated
    /// - If no dataset is provided for a type, a FIFO [`InMemDataset`] is created automatically
    ///
    /// # Type Parameters
    ///
    /// - `D`: The dataset implementation
    /// - `E`: The dataset's error type (must be convertible to [`Error`])
    /// - `T`: The type of data to store
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_core::Client;
    /// use spire_core::dataset::InMemDataset;
    ///
    /// #[derive(Clone)]
    /// struct Product {
    ///     name: String,
    ///     price: f64,
    /// }
    ///
    /// let client = Client::new(backend, worker)
    ///     .with_dataset(InMemDataset::<Product>::queue());
    /// ```
    ///
    /// [`InMemDataset`]: crate::dataset::InMemDataset
    /// [`Error`]: crate::Error
    pub fn with_dataset<D, E, T>(self, dataset: D) -> Self
    where
        D: Dataset<T, Error = E> + Clone + 'static,
        Error: From<E>,
        T: Send + Sync + 'static,
    {
        self.inner.datasets.set(dataset);
        self
    }

    /// Retrieves the [`Dataset`] for the specified type `T`.
    ///
    /// This method provides access to datasets from outside the worker context,
    /// allowing you to read scraped data after processing is complete.
    ///
    /// ## Behavior
    ///
    /// If no dataset has been registered for type `T`, a FIFO [`InMemDataset`] is
    /// automatically created, registered, and returned.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_core::Client;
    /// use spire_core::dataset::Dataset;
    ///
    /// #[derive(Clone)]
    /// struct Product {
    ///     name: String,
    ///     price: f64,
    /// }
    ///
    /// let client = Client::new(backend, worker);
    /// client.run().await?;
    ///
    /// // Retrieve scraped products
    /// let products = client.dataset::<Product>();
    /// while let Some(product) = products.read().await? {
    ///     println!("{}: ${}", product.name, product.price);
    /// }
    /// ```
    ///
    /// [`InMemDataset`]: crate::dataset::InMemDataset
    #[inline]
    pub fn dataset<T>(&self) -> Data<T>
    where
        T: Send + Sync + 'static,
    {
        Data::new(self.inner.datasets.get::<T>())
    }

    /// Returns a shutdown token for graceful shutdown.
    ///
    /// This token can be used to trigger graceful shutdown of the client from outside.
    /// When cancelled, the runner will stop processing new requests and finish current ones.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use tokio::time::{sleep, Duration};
    /// use spire_core::Client;
    ///
    /// let client = Client::new(backend, worker)
    ///     .with_initial_request(request);
    ///
    /// let shutdown = client.shutdown_token();
    ///
    /// // Spawn runner in background
    /// let handle = tokio::spawn(async move {
    ///     client.run().await
    /// });
    ///
    /// // Trigger shutdown after 5 seconds
    /// sleep(Duration::from_secs(5)).await;
    /// shutdown.cancel();
    ///
    /// // Wait for graceful shutdown
    /// handle.await.unwrap().unwrap();
    /// ```
    pub fn shutdown_token(&self) -> tokio_util::sync::CancellationToken {
        self.inner.shutdown_token()
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
        f.debug_struct("Client")
            .field("Datasets", &self.inner.datasets.len())
            .finish_non_exhaustive()
    }
}
