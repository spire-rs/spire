//! Core traits and utilities for data retrieval backends.
//!
//! This module provides the foundational traits for implementing backends and clients
//! in the Spire web scraping framework.
//!
//! # Core Traits
//!
//! - [`Backend`] - Creates and manages client instances
//! - [`Client`] - Fetches responses for HTTP requests
//! - [`Worker`] - Processes contexts and returns flow control
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
//! [`Trace`]: utils::Trace
//! [`Metric`]: utils::Metric
//! [`Noop`]: utils::Noop

use tower::{Service, ServiceExt};

use crate::{Error, Result};

mod client;
pub mod utils;
mod worker;

pub use client::Client;
pub use worker::Worker;

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
    async fn connect(&self) -> Result<Self::Client>;
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
    async fn connect(&self) -> Result<Self::Client> {
        let mut copy = self.clone();
        let ready = copy.ready().await?;
        ready.call(()).await
    }
}
