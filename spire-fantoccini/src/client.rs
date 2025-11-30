use std::ops::Deref;
use std::sync::Arc;
use std::task::{Context, Poll};

use deadpool::managed::Object;
use fantoccini::Client as WebClient;
use futures::future::BoxFuture;
use futures::FutureExt;
use tower::Service;

use spire_core::context::{Body, Request, Response};
use spire_core::{Error, Result};

use crate::manager::BrowserManager;

/// Client for interacting with a browser instance from the [`BrowserPool`].
///
/// This type wraps a pooled Fantoccini client and implements the Tower `Service`
/// trait for processing HTTP requests. It automatically dereferences to the
/// underlying `fantoccini::Client` for direct browser control.
///
/// # Examples
///
/// ```ignore
/// use spire_fantoccini::BrowserPool;
/// use spire_core::backend::Backend;
///
/// let pool = BrowserPool::builder()
///     .with_unmanaged("127.0.0.1:4444")
///     .build();
///
/// let client = pool.client().await?;
/// // Access fantoccini::Client methods directly
/// client.goto("https://example.com").await?;
/// ```
///
/// [`BrowserPool`]: crate::BrowserPool
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
    fn call(&mut self, _req: Request) -> Self::Future {
        let fut = async move {
            // TODO: Implement actual browser navigation and response extraction
            let response = Response::new(Body::default());
            Ok::<Response, Error>(response)
        };

        fut.boxed()
    }
}
