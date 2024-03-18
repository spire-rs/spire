//! Types and traits for data retrieval [`Backend`]s.
//!
//! ### Core
//!
//! - [`Backend`] is a core trait used to instantiate [`Client`]s.
//! - [`BrowserBackend`] is an extension trait for [`Backend`]s that run actual web browsers.
//! - [`Client`] is a core trait used to fetch [`Response`]s with [`Request`]s.
//!
//! ### Backend
//!
//! - [`HttpClient`] is a simple http client backed by the underlying [`Service`].
//! It is both [`Backend`] and [`Client`].
//! - [`BrowserPool`] is a [`Backend`] built on top of [`fantoccini`] crate. Uses
//! [`BrowserClient`] as a [`Client`].
//!

use tower::{Service, ServiceExt};

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub use client::{HttpClient, HttpClientBuilder};
#[cfg(feature = "driver")]
#[cfg_attr(docsrs, doc(cfg(feature = "driver")))]
pub use driver::{BrowserClient, BrowserManager, BrowserPool};

use crate::context::{Request, Response};
use crate::{Error, Result};

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
mod client;
#[cfg(feature = "driver")]
#[cfg_attr(docsrs, doc(cfg(feature = "driver")))]
mod driver;

/// Core trait used to instantiate [`Client`]s.
///
/// It is automatically implemented for cloneable [`Service`]s that return [`Client`]s.
#[async_trait::async_trait]
pub trait Backend: Clone + Send + Sized + 'static {
    /// Associated client type.
    type Client: Client;

    /// Returns a [`Self::Client`] from the pool.
    async fn client(&self) -> Result<Self::Client>;
}

#[async_trait::async_trait]
impl<S, T> Backend for S
where
    S: Service<(), Response = T, Error = Error>,
    S: Clone + Send + Sync + 'static,
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

/// Extension trait for [`Backend`]s that manage actual browsers.
///
/// Currently works as a marker trait only.
pub trait BrowserBackend: Backend {}

/// Core trait used to fetch [`Response`]s with [`Request`]s.
///
/// It is automatically implemented for cloneable [`Service`]s that take [`Request`]
/// and return [`Result`]<[`Response`]>.
#[async_trait::async_trait]
pub trait Client: Send + Sized + 'static {
    /// Tries to fetch the [`Response`].
    async fn resolve(self, req: Request) -> Result<Response>;
}

#[async_trait::async_trait]
impl<S> Client for S
where
    S: Service<Request, Response = Response, Error = Error>,
    S: Clone + Send + 'static,
    S::Future: Send + 'static,
{
    #[inline]
    async fn resolve(self, req: Request) -> Result<Response> {
        let mut copy = self.clone();
        let ready = copy.ready().await?;
        ready.call(req).await
    }
}
