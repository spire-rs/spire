//! Types and traits for data retrieval [`Backend`]s.
//!

use tower::Service;

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub use client::HttpClient;
#[cfg(feature = "driver")]
#[cfg_attr(docsrs, doc(cfg(feature = "driver")))]
pub use driver::{BrowserBackend, BrowserClient, BrowserManager, BrowserPool};

use crate::context::{Request, Response};
use crate::Error;

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub mod client;
#[cfg(feature = "driver")]
#[cfg_attr(docsrs, doc(cfg(feature = "driver")))]
pub mod driver;

/// TODO.
pub trait Backend: Service<Request, Response = Response, Error = Error> {
    /// TODO.
    type Client;
}
