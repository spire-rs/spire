//! Types and traits for data retrieval [`Backend`]s.
//!
//! ### Backends
//!
//! - [`Backend`] and [`BrowserBackend`]
//! - [`HttpClient`]
//! - [`BrowserPool`]
//!
//! ### Daemon
//!
//! - [`Router`]
//! - [`Daemon`] and [`DaemonHandle`]
//!

use std::convert::Infallible;

use tower::{Service, ServiceExt};

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub use client::{HttpClient, HttpClientBuilder};
pub use daemon::{Daemon, DaemonHandle};
#[cfg(feature = "driver")]
#[cfg_attr(docsrs, doc(cfg(feature = "driver")))]
pub use driver::{BrowserClient, BrowserManager, BrowserPool};

use crate::{Error, Result};
use crate::context::{Context as Cx, Request, Response, Signal};

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub mod client;
mod daemon;
#[cfg(feature = "driver")]
#[cfg_attr(docsrs, doc(cfg(feature = "driver")))]
pub mod driver;

/// TODO.
#[async_trait::async_trait]
pub trait Backend: Clone + Send + Sized + 'static {
    /// Associated client type.
    type Client: Client ;

    /// Returns a [`Self::Client`] from the pool.
    async fn client(&self) -> Result<Self::Client>;
}

#[async_trait::async_trait]
impl<S, T> Backend for S
where
    S: Service<(), Response = T, Error = Error>,
    S: Clone + Send + Sync + 'static,
    S::Future: Send + 'static,
    T: Client ,
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

/// TODO.
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

/// TODO: Rename.
#[async_trait::async_trait]
pub trait Router<B>: Clone + Send + 'static {
    /// TODO.
    async fn route(self, cx: Cx<B>) -> Signal;
}

#[async_trait::async_trait]
impl<S, B> Router<B> for S
where
    S: Service<Cx<B>, Response = Signal, Error = Infallible>,
    S: Clone + Send  + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
{
    #[inline]
    async fn route(self, cx: Cx<B>) -> Signal {
        let mut copy = self.clone();
        let ready = copy.ready().await.unwrap();
        ready.call(cx).await.unwrap()
    }
}
