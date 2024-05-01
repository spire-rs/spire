use std::fmt;
use std::ops::Deref;
use std::sync::Arc;
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
/// Implements [`Deref`] to `fantoccini::`[`Client`].
///
/// [`BrowserPool`]: crate::backend::BrowserPool
#[derive(Clone)]
pub struct BrowserClient(Arc<Object<BrowserManager>>);

impl BrowserClient {
    /// Creates a new [`BrowserClient`].
    #[inline]
    pub(crate) fn new(inner: Object<BrowserManager>) -> Self {
        Self(Arc::new(inner))
    }
}

impl fmt::Debug for BrowserClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0.client, f)
    }
}

impl Deref for BrowserClient {
    type Target = Client;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0.client
    }
}

impl Service<Request> for BrowserClient {
    type Response = Response;
    type Error = Error;
    type Future = BoxFuture<'static, Result<Response>>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: Request) -> Self::Future {
        todo!()
    }
}
