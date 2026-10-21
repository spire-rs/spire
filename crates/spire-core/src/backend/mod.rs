//! Backends and the clients they hand out.
//!
//! A [`Backend`] is shared by the whole crawl and hands out one [`Client`]
//! per request: an HTTP client, a pooled browser session, or anything else
//! that turns a [`Request`] into a [`Response`].

mod noop;
mod rotate;
mod status;

use std::future::Future;

use bytes::Bytes;
pub use noop::Noop;
pub use rotate::Rotate;
pub use status::StatusPolicy;

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

/// Loads the page of the one [`Request`] it is handed out for.
///
/// [`fetch`](Self::fetch) sends the request, or navigates to it, and returns
/// the response's head: its status, headers and final URI.
/// [`body`](Self::body) then returns the page's content: the bytes received
/// for an HTTP client, or the page as currently rendered for a browser.
/// [`Context`](crate::Context) fetches once and keeps the head; a failed
/// fetch may be tried again.
pub trait Client: Send + Sync + 'static {
    /// Sends `request`, or navigates to it, returning the response's head.
    ///
    /// # Errors
    ///
    /// Returns an error if the request cannot be sent or the transfer fails.
    fn fetch(&self, request: &Request) -> impl Future<Output = Result<Response<()>>> + Send;

    /// Returns the content of the fetched page.
    ///
    /// # Errors
    ///
    /// Returns an error if nothing was fetched yet or the content cannot be
    /// read.
    fn body(&self) -> impl Future<Output = Result<Bytes>> + Send;
}
