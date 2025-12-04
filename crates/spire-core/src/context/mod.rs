//! Request context and extensions for web scraping.
//!
//! This module provides the [`Context`] type that wraps HTTP requests with additional
//! metadata and utilities for web scraping, including:
//!
//! - Request/response body handling
//! - Request queuing and routing via tags
//! - FlowControl-based flow control
//! - Dataset access for storing scraped data

mod flow_control;
mod http_body;
mod request_extend;
mod request_queue;
mod request_source;

use std::fmt;

pub use flow_control::{FlowControl, IntoFlowControl, TagQuery};
pub use http_body::{Body, Request, Response};
pub use request_extend::{Depth, Tag, Task, TaskBuilder, TaskExt};
pub use request_queue::RequestQueue;
pub use request_source::RequestSource;

use crate::Result;
use crate::backend::Client;
use crate::dataset::{Data, DatasetRegistry};

/// Framework-specific context of the [`Request`].
pub struct Context<C> {
    request: Request,
    client: C,
    datasets: DatasetRegistry,
}

impl<C> Context<C> {
    /// Creates a new [`Context`].
    pub const fn new(request: Request, client: C, datasets: DatasetRegistry) -> Self {
        Self {
            request,
            client,
            datasets,
        }
    }

    /// Resolves the [`Request`] and returns [`Response`] or [`Error`].
    ///
    /// [`Error`]: crate::Error
    pub async fn resolve(self) -> Result<Response>
    where
        C: Client,
    {
        let response = self.client.resolve(self.request).await?;
        Ok(response)
    }

    /// Returns the reference to the inner [`Request`].
    ///
    /// Used by extractors to access extensions.
    pub const fn get_ref(&self) -> &Request {
        &self.request
    }

    /// Returns the mutable reference to the inner [`Request`].
    ///
    /// Used by extractors to access extensions.
    pub fn get_mut(&mut self) -> &mut Request {
        &mut self.request
    }

    /// Returns the reference to the [`Backend`]'s client.
    ///
    /// [`Backend`]: crate::backend::Backend
    pub const fn as_client_ref(&self) -> &C {
        &self.client
    }

    /// Returns the mutable reference to the [`Backend`]'s client.
    ///
    /// [`Backend`]: crate::backend::Backend
    pub fn as_client_mut(&mut self) -> &mut C {
        &mut self.client
    }

    /// Initializes and returns the [`RequestQueue`].
    pub fn request_queue(&self) -> RequestQueue {
        use std::num::NonZeroU32;
        let dataset = self.datasets.get::<Request>();
        let depth = NonZeroU32::new(self.request.depth()).unwrap_or(NonZeroU32::MIN);
        let mut queue = RequestQueue::new(dataset, depth);

        // Attach existing tag from request as default
        if !self.request.tag().is_fallback() {
            queue = queue.with_default_tag(self.request.tag().clone());
        }

        // Attach existing depth from request as default
        queue = queue.with_default_depth(Depth::new(self.request.depth()));

        queue
    }

    /// Initializes and returns the boxed [`Dataset`] of type `T`.
    ///
    /// [`Dataset`]: crate::dataset::Dataset
    pub fn dataset<T>(&self) -> Data<T>
    where
        T: Send + Sync + 'static,
    {
        Data::new(self.datasets.get::<T>())
    }
}

impl<B> fmt::Debug for Context<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context").finish_non_exhaustive()
    }
}
