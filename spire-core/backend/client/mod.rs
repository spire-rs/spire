use std::fmt;
use std::future::{ready, Ready};
use std::sync::Mutex;
use std::task::{Context, Poll};

use futures::future::BoxFuture;
use tower::util::BoxCloneService;
use tower::{Service, ServiceExt};

use crate::context::{Body, Request, Response};
use crate::{Error, Result};

/// Simple `http` client backed by the underlying [`Service`].
/// It is both [`Backend`] and [`Client`].
///
/// [`Backend`]: crate::backend::Backend
/// [`Client`]: crate::backend::Client
#[must_use = "services do nothing unless you `.poll_ready` or `.call` them"]
pub struct HttpClient {
    inner: Mutex<BoxCloneService<Request, Response, Error>>,
}

impl HttpClient {
    /// Creates a new [`HttpClient`].
    pub fn new<S, B, E>(svc: S) -> Self
    where
        S: Service<Request<B>, Response = Response<B>, Error = E> + Clone + Send + 'static,
        B: From<Body> + Into<Body>,
        S::Future: Send + 'static,
        E: Into<Error> + 'static,
    {
        let svc = svc
            .map_request(|x: Request| -> Request<B> { x.map(Into::into) })
            .map_response(|x: Response<B>| -> Response { x.map(Into::into) })
            .map_err(|x: E| -> Error { x.into() });

        let inner = Mutex::new(BoxCloneService::new(svc));
        Self { inner }
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        todo!()
    }
}

impl Clone for HttpClient {
    fn clone(&self) -> Self {
        let inner = {
            let svc = self.inner.lock().unwrap();
            Mutex::new(svc.clone())
        };

        Self { inner }
    }
}

impl fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpClient").finish_non_exhaustive()
    }
}

impl Service<()> for HttpClient {
    type Response = Self;
    type Error = Error;
    type Future = Ready<Result<Self::Response, Self::Error>>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, _req: ()) -> Self::Future {
        ready(Ok(self.clone()))
    }
}

impl Service<Request> for HttpClient {
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

#[cfg(test)]
mod test {
    use reqwest::{Client as RwClient, Request as RwRequest};
    use reqwest::{Error as RwError, Response as RwResponse};
    use tower::ServiceBuilder;

    use crate::backend::util::Noop;
    use crate::backend::HttpClient;
    use crate::context::{Request, Response};
    use crate::dataset::InMemDataset;
    use crate::{BoxError, Client, Result};

    #[test]
    fn service() {
        // BLOCKED: https://github.com/seanmonstar/reqwest/issues/2039
        // BLOCKED: https://github.com/seanmonstar/reqwest/pull/2060

        let svc = ServiceBuilder::default()
            .map_request(|_x: Request| -> RwRequest { unreachable!() })
            .map_response(|_x: RwResponse| -> Response { unreachable!() })
            .map_err(|x: RwError| -> BoxError { x.into() })
            .service(RwClient::default());

        let _ = HttpClient::new(svc);
    }

    #[tokio::test]
    #[cfg(feature = "tracing")]
    #[tracing_test::traced_test]
    async fn noop() -> Result<()> {
        use crate::backend::util::Trace;

        let backend = Trace::new(HttpClient::default());
        let worker = Trace::new(Noop::default());

        let request = Request::get("https://example.com/").body(());
        let client = Client::new(backend, worker)
            .with_request_queue(InMemDataset::stack())
            .with_dataset(InMemDataset::<u64>::new())
            .with_initial_request(request.unwrap());

        let _ = client.dataset::<u64>();
        let _ = client.run().await?;
        Ok(())
    }
}
