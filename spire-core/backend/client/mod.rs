//! TODO.
//!
//!

use std::fmt;
use std::sync::Mutex;
use std::task::{Context, Poll};

use futures::future::BoxFuture;
use tower::{Service, ServiceExt};
use tower::util::BoxCloneService;

pub use builder::HttpClientBuilder;
pub use handler::HttpClient;

use crate::{BoxError, Error, Result};
use crate::backend::Backend;
use crate::context::{Request, Response};

mod builder;
mod handler;

/// Simple http client [`Backend`]  backed by the underlying [`Service`].
pub struct HttpClientPool {
    inner: Mutex<BoxCloneService<Request, Response, Error>>,
}

impl HttpClientPool {
    /// Creates a new [`HttpClientPool`].
    pub fn new<S, E>(svc: S) -> Self
    where
        S: Service<Request, Response = Response, Error = E> + Clone + Send + 'static,
        S::Future: Send + 'static,
        E: Into<BoxError> + 'static,
    {
        let svc = svc.map_err(Error::new);
        let inner = Mutex::new(BoxCloneService::new(svc));
        Self { inner }
    }

    /// Creates a new [`HttpClientBuilder`].
    pub fn builder() -> HttpClientBuilder {
        HttpClientBuilder::new()
    }
}

impl Default for HttpClientPool {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl Clone for HttpClientPool {
    fn clone(&self) -> Self {
        let svc = self.inner.lock().unwrap();
        let inner = Mutex::new(svc.clone());
        Self { inner }
    }
}

impl fmt::Debug for HttpClientPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpClient").finish_non_exhaustive()
    }
}

impl Service<Request> for HttpClientPool {
    type Response = Response;
    type Error = Error;
    type Future = BoxFuture<'static, Result<Response>>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let mut guard = self.inner.lock().unwrap();
        guard.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, req: Request) -> Self::Future {
        let mut guard = self.inner.lock().unwrap();
        guard.call(req)
    }
}

#[async_trait::async_trait]
impl Backend for HttpClientPool {
    type Client = HttpClient;

    async fn call(&self) -> Result<Self::Client> {
        let svc = self.inner.lock().unwrap();
        Ok(HttpClient::new(svc.clone()))
    }
}

#[cfg(test)]
mod test {
    use reqwest::{Client, Error};
    use reqwest::{Request as RwRequest, Response as RwResponse};
    use tower::ServiceBuilder;

    use crate::backend::HttpClientPool;
    use crate::BoxError;
    use crate::context::{Request, Response};

    #[test]
    fn reqwest() {
        // BLOCKED: https://github.com/seanmonstar/reqwest/issues/2039
        // BLOCKED: https://github.com/seanmonstar/reqwest/pull/2060

        let svc = ServiceBuilder::default()
            .map_request(|_x: Request| -> RwRequest { unreachable!() })
            .map_response(|_x: RwResponse| -> Response { unreachable!() })
            .map_err(|x: Error| -> BoxError { x.into() })
            .service(Client::default());

        let _ = HttpClientPool::new(svc);
    }
}
