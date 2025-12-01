//! Core traits and utilities for data retrieval backends.
//!
//! This module provides the foundational traits for implementing backends and clients
//! in the Spire web scraping framework.
//!
//! # Core Traits
//!
//! - [`Backend`] - Creates and manages client instances
//! - [`Client`] - Fetches responses for HTTP requests
//! - [`Worker`] - Processes contexts and returns flow control signals
//!
//! # Concrete Implementations
//!
//! Concrete backend implementations are provided in separate crates:
//!
//! - `spire-reqwest` - HTTP client backend using Reqwest/Tower
//! - `spire-thirtyfour` - WebDriver browser automation backend
//!
//! # Utilities
//!
//! - [`Trace`] - Tracing middleware for backends, clients, and workers
//! - [`Metric`] - Metrics collection middleware for workers
//! - [`Noop`] - No-op implementations for testing and debugging
//!
//! [`Trace`]: util::Trace
//! [`Metric`]: util::Metric
//! [`Noop`]: util::Noop

use std::convert::Infallible;
use std::future::Future;

use tower::{Service, ServiceExt};

use crate::context::{Context, Request, Response, Signal};
use crate::{Error, Result};

pub mod utils;

/// Core trait for creating client instances.
///
/// A `Backend` is responsible for managing and providing [`Client`] instances
/// that can fetch HTTP responses. It is automatically implemented for cloneable
/// Tower services that return clients.
///
/// # Examples
///
/// ```ignore
/// use spire_core::backend::Backend;
///
/// async fn use_backend<B: Backend>(backend: B) {
///     let client = backend.client().await.unwrap();
///     // Use the client to fetch responses
/// }
/// ```
#[async_trait::async_trait]
pub trait Backend: Clone + Send + Sized + 'static {
    /// Associated client type.
    type Client: Client;

    /// Returns a [`Self::Client`] from the backend.
    async fn client(&self) -> Result<Self::Client>;
}

#[async_trait::async_trait]
impl<S, T> Backend for S
where
    S: Service<(), Response = T, Error = Error> + Clone + Send + Sync + 'static,
    S::Future: Send + 'static,
    T: Client,
{
    type Client = T;

    #[inline]
    async fn client(&self) -> Result<Self::Client> {
        let mut copy = self.clone();
        let ready = copy.ready().await?;
        ready.call(()).await
    }
}

/// Core trait for fetching HTTP responses.
///
/// A `Client` handles individual HTTP requests and returns responses.
/// It is automatically implemented for cloneable Tower services that
/// handle [`Request`]s and return [`Result`]<[`Response`]>.
///
/// # Examples
///
/// ```ignore
/// use spire_core::backend::Client;
/// use spire_core::context::Request;
///
/// async fn fetch<C: Client>(client: C, request: Request) {
///     let response = client.resolve(request).await.unwrap();
///     // Process the response
/// }
/// ```
#[async_trait::async_trait]
pub trait Client: Send + Sized + 'static {
    /// Fetches the [`Response`] for the given request.
    async fn resolve(self, req: Request) -> Result<Response>;
}

#[async_trait::async_trait]
impl<S> Client for S
where
    S: Service<Request, Response = Response, Error = Error> + Send + 'static,
    S::Future: Send + 'static,
{
    #[inline]
    async fn resolve(mut self, req: Request) -> Result<Response> {
        let ready = self.ready().await?;
        ready.call(req).await
    }
}

/// Core trait for processing scraping tasks.
///
/// A `Worker` processes a [`Context`] (containing a request, client, and datasets)
/// and returns a [`Signal`] that controls execution flow. It is automatically
/// implemented for cloneable Tower services.
///
/// # Examples
///
/// ```ignore
/// use spire_core::backend::Worker;
/// use spire_core::context::{Context, Signal};
///
/// #[derive(Clone)]
/// struct MyWorker;
///
/// impl<C> Worker<C> for MyWorker {
///     async fn invoke(self, cx: Context<C>) -> Signal {
///         // Process the context
///         Signal::Continue
///     }
/// }
/// ```
///
/// [`Context`]: crate::context::Context
pub trait Worker<C>: Clone + Send + 'static {
    /// Processes the context and returns a flow control signal.
    ///
    /// ## Note
    ///
    /// This method consumes `self` due to current Tower service requirements.
    /// Future versions may use `&self` instead.
    fn invoke(self, cx: Context<C>) -> impl Future<Output = Signal>;
}

impl<S, C> Worker<C> for S
where
    S: Service<Context<C>, Response = Signal, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send + 'static,
    C: Send + 'static,
{
    #[inline]
    async fn invoke(self, cx: Context<C>) -> Signal {
        let mut this = self.clone();
        let ready = this.ready().await.expect("Worker should be infallible");
        ready.call(cx).await.expect("Worker should be infallible")
    }
}
