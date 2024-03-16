use std::fmt;
use std::task::{Context, Poll};

use futures::future::BoxFuture;
use tower::util::BoxCloneService;
use tower::{Service, ServiceExt};

use crate::backend::Client;
use crate::context::{Request, Response};
use crate::{Error, Result};

pub struct HttpClient {
    inner: BoxCloneService<Request, Response, Error>,
}

impl HttpClient {
    /// Creates a new [`HttpClient`].
    pub(crate) fn new(svc: BoxCloneService<Request, Response, Error>) -> Self {
        Self { inner: svc }
    }
}

impl fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpClient").finish_non_exhaustive()
    }
}

impl Service<Request> for HttpClient {
    type Response = Response;
    type Error = Error;
    type Future = BoxFuture<'static, crate::Result<Response>>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, req: Request) -> Self::Future {
        self.inner.call(req)
    }
}

#[async_trait::async_trait]
impl Client for HttpClient {
    #[inline]
    async fn invoke(self, req: Request) -> Result<Response> {
        self.oneshot(req).await
    }
}
