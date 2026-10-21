//! Backends and the clients they hand out.
//!
//! A [`Backend`] is shared by the whole crawl and hands out one [`Client`]
//! per request: an HTTP client, a pooled browser session, or anything else
//! that turns a [`Request`] into a [`Response`].

mod noop;

use std::future::Future;

pub use noop::Noop;

use crate::Result;
use crate::request::{Request, Response};

/// Hands out a [`Client`] for every request of a crawl.
pub trait Backend: Send + Sync + 'static {
    /// The client used to fetch a single request.
    type Client: Client;

    /// Returns a client for the next request.
    ///
    /// # Errors
    ///
    /// Returns an error if no client can be created, e.g. when a pool is
    /// exhausted or a browser fails to start.
    fn connect(&self) -> impl Future<Output = Result<Self::Client>> + Send;
}

/// Fetches the [`Response`] for a single [`Request`].
pub trait Client: Send + 'static {
    /// Fetches the response for the request, consuming the client.
    ///
    /// # Errors
    ///
    /// Returns an error if the request cannot be sent or the transfer fails.
    fn fetch(self, request: Request) -> impl Future<Output = Result<Response>> + Send;
}
