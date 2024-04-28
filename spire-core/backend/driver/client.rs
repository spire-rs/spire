use std::fmt;
use std::ops::{Deref, DerefMut};
use std::task::{Context, Poll};

use deadpool::managed::Object;
use fantoccini::Client;
use futures::future::BoxFuture;
use tower::Service;

use crate::{Error, Result};
use crate::backend::BrowserManager;
use crate::context::{Request, Response};

/// [`BrowserPool`] client.
///
/// Implements [`Deref`] and [`DerefMut`] to `fantoccini::`[`Client`].
///
/// [`BrowserPool`]: crate::backend::BrowserPool
pub struct BrowserClient {
    inner: Object<BrowserManager>,
}

impl BrowserClient {
    /// Creates a new [`BrowserClient`].
    pub(crate) fn new(inner: Object<BrowserManager>) -> Self {
        Self { inner }
    }
}

impl fmt::Debug for BrowserClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner.client, f)
    }
}

impl Deref for BrowserClient {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        &self.inner.client
    }
}

impl DerefMut for BrowserClient {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner.client
    }
}

impl Service<Request> for BrowserClient {
    type Response = Response;
    type Error = Error;
    type Future = BoxFuture<'static, Result<Response>>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        todo!()
    }

    #[inline]
    fn call(&mut self, req: Request) -> Self::Future {
        todo!()
    }
}
