//! Backends and the clients they hand out.
//!
//! A [`Backend`] is shared by the whole crawl and hands out one [`Client`]
//! per request: an HTTP client, a pooled browser session, or anything else
//! that turns a [`Request`] into a [`Response`].

mod noop;
mod status;

use std::future::Future;

use bytes::Bytes;
pub use noop::Noop;
pub use status::StatusPolicy;

use crate::Result;
use crate::identity::Identity;
use crate::request::Request;
use crate::response::Response;

/// Hands out a [`Client`] for every request of a crawl.
pub trait Backend: Send + Sync + 'static {
    /// The client used to fetch a single request.
    type Client: Client;

    /// Returns a client for the next request, sent directly, from the
    /// crawler itself, with the backend's own cookies and sessions.
    ///
    /// # Errors
    ///
    /// Returns an error if no client can be created, e.g. when a pool is
    /// exhausted or a browser fails to start.
    fn connect(&self) -> impl Future<Output = Result<Self::Client>> + Send;

    /// Returns a client for the next request, which goes out as `identity`,
    /// for a crawl with [identities](crate::identity).
    ///
    /// The client must send its request through the identity's
    /// [proxy](Identity::proxy), and keep each identity's cookies and
    /// session state apart from every other's, so that one identity always
    /// looks like one visitor. The identity's [headers](Identity::headers)
    /// are already on the request.
    ///
    /// By default, an identity without a proxy connects as
    /// [`connect`](Self::connect) does, and one with a proxy is refused, so
    /// a request is never sent from the crawler's own IP by mistake.
    /// Backends that route through proxies, or keep cookies or sessions,
    /// override it.
    ///
    /// # Errors
    ///
    /// Returns an [`Invalid`](crate::ErrorKind::Invalid) error if the
    /// identity's proxy is not supported, or an error if no client can be
    /// created.
    fn connect_as(&self, identity: &Identity) -> impl Future<Output = Result<Self::Client>> + Send {
        let supported = identity.require_no_proxy();
        async move {
            supported?;
            self.connect().await
        }
    }
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
