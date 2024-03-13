use std::fmt;
use std::future::Ready;
use std::sync::Mutex;
use std::task::{Context, Poll};

use tower::util::BoxCloneService;
use tower::{Service, ServiceExt};

use builder::Builder;

use crate::backend::Backend;
use crate::context::{Body, Request, Response};
use crate::{BoxError, Error};

mod builder;

pub struct HttpClient {
    inner: Mutex<BoxCloneService<Request, Response, Error>>,
}

impl HttpClient {
    /// Creates a new [`HttpClient`].
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

    pub fn new2<S, B, E, ETryInto, ETryFrom>(svc: S) -> Self
    where
        S: Service<Request<B>, Response = Response<B>, Error = E> + Clone + Send + 'static,
        S::Future: Send + 'static,
        E: Into<BoxError>,
        B: TryInto<Body, Error = ETryInto> + TryFrom<Body, Error = ETryFrom>,
        ETryInto: Into<BoxError>,
        ETryFrom: Into<BoxError>,
    {
        // TODO: remap Body.
        todo!()
    }

    pub fn builder() -> Builder {
        Builder::new()
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        todo!()
    }
}

impl Clone for HttpClient {
    fn clone(&self) -> Self {
        let svc = self.inner.lock().unwrap();
        let inner = Mutex::new(svc.clone());
        Self { inner }
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
    type Future = Ready<Result<Response, Error>>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: Request) -> Self::Future {
        todo!()
    }
}

#[async_trait::async_trait]
impl Backend for HttpClient {
    type Client = ();
}

#[cfg(test)]
mod test {
    use reqwest::{Client, Error};
    use reqwest::{Request as RwRequest, Response as RwResponse};
    use tower::ServiceBuilder;

    use crate::backend::HttpClient;
    use crate::context::{Request, Response};
    use crate::BoxError;

    #[test]
    fn reqwest() {
        // BLOCKED: https://github.com/seanmonstar/reqwest/issues/2039
        // BLOCKED: https://github.com/seanmonstar/reqwest/pull/2060

        let svc = ServiceBuilder::default()
            .map_request(|x: Request| -> RwRequest { todo!() })
            .map_response(|x: RwResponse| -> Response { todo!() })
            .map_err(|x: Error| -> BoxError { x.into() })
            .service(Client::default());

        let _ = HttpClient::new(svc);
    }
}
