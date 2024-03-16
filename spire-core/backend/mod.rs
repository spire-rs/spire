//! Types and traits for data retrieval [`Backend`]s.
//!

use async_trait::async_trait;
use tower::Service;

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub use client::{HttpClient, HttpClientBuilder, HttpClientPool};
#[cfg(feature = "driver")]
#[cfg_attr(docsrs, doc(cfg(feature = "driver")))]
pub use driver::{BrowserBackend, BrowserClient, BrowserManager, BrowserPool};

use crate::context::{Request, Response};
use crate::{Error, Result};

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub mod client;
#[cfg(feature = "driver")]
#[cfg_attr(docsrs, doc(cfg(feature = "driver")))]
pub mod driver;

/// TODO.
#[async_trait]
pub trait Backend {
    /// TODO.
    type Client: Service<Request, Response = Response, Error = Error>;

    async fn instance(&self) -> Result<Self::Client>;
}
