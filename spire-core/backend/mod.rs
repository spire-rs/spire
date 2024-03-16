//! Types and traits for data retrieval [`Backend`]s.
//!
//! ### Backends
//!
//! - [`HttpClientPool`]
//! - [`BrowserPool`]
//!

use std::future::Future;

use tower::Service;

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub use client::{HttpClient, HttpClientBuilder, HttpClientPool};
#[cfg(feature = "driver")]
#[cfg_attr(docsrs, doc(cfg(feature = "driver")))]
pub use driver::{BrowserClient, BrowserManager, BrowserPool};

use crate::context::{Request, Response};
use crate::Result;

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub mod client;
#[cfg(feature = "driver")]
#[cfg_attr(docsrs, doc(cfg(feature = "driver")))]
pub mod driver;

/// TODO.
#[async_trait::async_trait]
pub trait Backend: Send + Sized + 'static {
    /// Associated client type.
    type Client: Client + Send + 'static;

    /// Returns a [`Self::Client`] from the pool.
    async fn call(&self) -> Result<Self::Client>;
}

/// Extension trait for [`Backend`]s that manage actual browsers.
///
/// Currently works as a marker trait only.
pub trait BrowserBackend: Backend {}

/// TODO.
#[async_trait::async_trait]
pub trait Client: Sized + 'static {
    /// Attempts to retrieve the [`Response`].
    async fn invoke(self, req: Request) -> Result<Response>;
}
