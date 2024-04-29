//! Types and traits for data retrieval [`Backend`]s.
//!
//! ### Core
//!
//! - [`Backend`] is a core trait used to instantiate [`Client`]s.
//! - [`Client`] is a core trait used to fetch [`Response`]s with [`Request`]s.
//! - [`Worker`] is a core trait used to process [`Context`]s and return [`Signal`]s.
//!
//! ### Backend
//!
//! - [`HttpClient`] is a simple `http` client backed by the underlying
//! `tower::`[`Service`]. It is both [`Backend`] and [`Client`].
//! - [`BrowserPool`] is a [`Backend`] built on top of [`fantoccini`] crate.
//! Uses [`BrowserClient`] as a [`Client`].
//!
//! ### Utility
//!
//! - [`TraceEntity`] is a tracing wrapper for [`Backend`]s, [`Client`]s and [`Worker`]s,
//! used for improved observability.
//! - [`DebugEntity`] is a no-op [`Backend`], [`Client`] and [`Worker`], used for
//! testing and debugging.
//!
//! [`TraceEntity`]: util::TraceEntity
//! [`DebugEntity`]: util::DebugEntity

use std::convert::Infallible;

use tower::{Service, ServiceExt};

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub use client::HttpClient;
#[cfg(feature = "driver")]
#[cfg_attr(docsrs, doc(cfg(feature = "driver")))]
pub use driver::{BrowserClient, BrowserManager, BrowserPool};

use crate::context::{Context, Request, Response, Signal};
use crate::{Error, Result};

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
mod client;
#[cfg(feature = "driver")]
#[cfg_attr(docsrs, doc(cfg(feature = "driver")))]
mod driver;
pub mod util;

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
    S: Service<(), Response = T, Error = Error> + Clone + Send + Sync + 'static,
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

/// Core trait used to retrieve [`Response`]s with [`Request`]s.
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
    S: Service<Request, Response = Response, Error = Error> + Send + 'static,
    S::Future: Send + 'static,
{
    #[inline]
    async fn resolve(mut self, req: Request) -> Result<Response> {
        let ready = self.ready().await?;
        ready.call(req).await
    }
}

/// Core trait used to process [`Context`]s and return [`Signal`]s.
///
/// It is automatically implemented for cloneable [`Service`]s that take [`Context`].
///
/// [`Context`]: crate::context::Context
#[async_trait::async_trait]
pub trait Worker<B>: Clone + Send + 'static {
    /// TODO: Remove Clone + replace self with &self.
    async fn invoke(self, cx: Context<B>) -> Signal;
}

#[async_trait::async_trait]
impl<S, B> Worker<B> for S
where
    S: Service<Context<B>, Response = Signal, Error = Infallible>,
    S: Clone + Send + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
{
    #[inline]
    async fn invoke(self, cx: Context<B>) -> Signal {
        let mut copy = self.clone();
        let ready = copy.ready().await.unwrap();
        ready.call(cx).await.unwrap()
    }
}
