//! Backends and the sessions they hand out.
//!
//! A [`Backend`] is shared by the whole crawl and hands out one [`Session`]
//! per request: an HTTP client, a pooled browser session, or anything else
//! that turns a [`Request`] into a [`Response`].

pub mod document;
mod noop;
mod status;

use std::future::Future;

pub use noop::Noop;
pub use status::StatusPolicy;

use crate::Result;
use crate::http::{Request, Response};
use crate::identity::Identity;

/// Hands out a [`Session`] for every request of a crawl.
pub trait Backend: Send + Sync + 'static {
    /// The session used to fetch a single request.
    type Session: Session;

    /// Returns a session for the next request, sent directly, from the
    /// crawler itself, with the backend's own cookies and sessions.
    ///
    /// # Errors
    ///
    /// Returns an error if no session can be created, e.g. when a pool is
    /// exhausted or a browser fails to start.
    fn connect(&self) -> impl Future<Output = Result<Self::Session>> + Send;

    /// Returns a session for the next request, which goes out as `identity`,
    /// for a crawl with [identities](crate::identity).
    ///
    /// The session must send its request through the identity's
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
    /// identity's proxy is not supported, or an error if no session can be
    /// created.
    fn connect_as(
        &self,
        identity: &Identity,
    ) -> impl Future<Output = Result<Self::Session>> + Send {
        let supported = identity.require_no_proxy();
        async move {
            supported?;
            self.connect().await
        }
    }
}

/// Fetches one request using a backend's resources.
///
/// A successful fetch returns a complete response with an immutable `Bytes`
/// body. HTTP sessions return the received bytes; browser sessions return
/// an HTML snapshot captured after navigation. A live browser document is
/// a separate optional capability, accessed through [`Self::document`].
/// [`Context`](crate::Context) caches the first successful response.
pub trait Session: Send + Sync + 'static {
    /// Fetches `request`, returning its status, headers, final URI and body.
    ///
    /// # Errors
    ///
    /// Returns an error if fetching, status checking or reading the body fails.
    fn fetch(&self, request: &Request) -> impl Future<Output = Result<Response>> + Send;

    /// Returns the live document capability, if this session has a DOM.
    ///
    /// This reports support, not navigation state. Operations on the
    /// document may fail before a page is fetched. Fetch through the
    /// context before querying it; capture failures must not silently
    /// fall back to parsing the response snapshot.
    fn document(&self) -> Option<&dyn document::Document> {
        None
    }
}
