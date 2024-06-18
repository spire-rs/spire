use std::ops::Deref;
use std::sync::Arc;
use std::task::{Context, Poll};

use deadpool::managed::Object;
use fantoccini::Client as WebClient;
use futures::future::BoxFuture;
use futures::FutureExt;
use tower::Service;

use crate::backend::driver::BrowserManager;
use crate::context::{Body, Request, Response};
use crate::{Error, Result};

/// [`BrowserPool`] client.
///
/// Implements [`Deref`] to `fantoccini::`[`Client`].
///
/// [`BrowserPool`]: crate::backend::BrowserPool
/// [`Client`]: WebClient
#[derive(Clone)]
pub struct BrowserClient(Arc<Object<BrowserManager>>);

impl From<Object<BrowserManager>> for BrowserClient {
    #[inline]
    fn from(value: Object<BrowserManager>) -> Self {
        Self(Arc::new(value))
    }
}

impl Deref for BrowserClient {
    type Target = WebClient;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Service<Request> for BrowserClient {
    type Response = Response;
    type Error = Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: Request) -> Self::Future {
        let fut = async move {
            let response = Response::new(Body::default());
            Ok::<Response, Error>(response)
        };

        fut.boxed()
    }
}
