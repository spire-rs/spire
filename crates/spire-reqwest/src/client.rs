use std::fmt;
use std::future::{Ready, ready};
use std::sync::Mutex;
use std::task::{Context, Poll};

use futures::future::BoxFuture;
use spire_core::context::{Body, Request, Response};
use spire_core::{Error, Result};
use tower::util::BoxCloneService;
use tower::{Service, ServiceExt};

/// Converts an http::Request to a reqwest::Request.
fn request_to_reqwest(req: Request) -> reqwest::Request {
    use reqwest::Client as RwClient;

    let (parts, _body) = req.into_parts();

    // TODO: Handle request body properly - requires async or streaming
    let body_bytes = bytes::Bytes::new();

    // Build URL from URI
    let url = parts.uri.to_string();

    // Build reqwest request
    RwClient::new()
        .request(parts.method, &url)
        .headers(parts.headers)
        .version(parts.version)
        .body(body_bytes)
        .build()
        .expect("failed to build request")
}

/// Converts a reqwest::Response to an http::Response.
fn response_from_reqwest(rw_res: reqwest::Response) -> Response {
    // Convert reqwest::Response to http::Response
    let mut res_builder = Response::builder()
        .status(rw_res.status())
        .version(rw_res.version());

    if let Some(headers) = res_builder.headers_mut() {
        *headers = rw_res.headers().clone();
    }

    // TODO: Handle response body properly - requires async streaming
    let body = Body::from(bytes::Bytes::new());

    res_builder.body(body).expect("failed to build response")
}

/// Simple HTTP client backed by an underlying Tower [`Service`].
///
/// `HttpClient` wraps any Tower service that can handle HTTP requests and responses,
/// making it compatible with the Spire backend system. It implements both
/// [`Backend`] and [`Client`] traits.
///
/// # Examples
///
/// ```ignore
/// use spire_reqwest::HttpClient;
/// use reqwest::Client as ReqwestClient;
/// use tower::ServiceBuilder;
///
/// // Wrap a reqwest client
/// let svc = ServiceBuilder::default()
///     .service(ReqwestClient::default());
///
/// let http_client = HttpClient::new(svc);
/// ```
///
/// [`Backend`]: spire_core::backend::Backend
/// [`Client`]: spire_core::backend::Client
#[must_use = "services do nothing unless you `.poll_ready` or `.call` them"]
pub struct HttpClient {
    inner: Mutex<BoxCloneService<Request, Response, Error>>,
}

impl HttpClient {
    /// Creates a new [`HttpClient`] from a Tower service.
    ///
    /// # Type Parameters
    ///
    /// - `S`: The underlying Tower service
    /// - `B`: The body type used by the service
    /// - `E`: The error type from the service (must convert to [`Error`])
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_reqwest::HttpClient;
    /// use tower::ServiceBuilder;
    ///
    /// let svc = ServiceBuilder::default()
    ///     .service(my_http_service);
    ///
    /// let client = HttpClient::new(svc);
    /// ```
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
    /// Creates a default HTTP client using a default reqwest client.
    ///
    /// This creates a basic HTTP client with default configuration.
    /// For custom configuration, use [`HttpClient::new`] with a configured service.
    fn default() -> Self {
        use reqwest::Client as RwClient;
        use tower::ServiceBuilder;

        let svc = ServiceBuilder::default()
            .map_request(request_to_reqwest)
            .map_response(response_from_reqwest)
            .map_err(|x: reqwest::Error| -> Error { Error::from_boxed(x) })
            .service(RwClient::default());

        Self::new(svc)
    }
}

impl Clone for HttpClient {
    fn clone(&self) -> Self {
        let inner = {
            let svc = self.inner.lock().expect("HttpClient mutex poisoned");
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
    type Error = Error;
    type Future = Ready<Result<Self::Response, Self::Error>>;
    type Response = Self;

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
    type Error = Error;
    type Future = BoxFuture<'static, Result<Response>>;
    type Response = Response;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let mut guard = self.inner.lock().expect("HttpClient mutex poisoned");
        guard.poll_ready(cx)
    }

    #[inline]
    fn call(&mut self, req: Request) -> Self::Future {
        let mut guard = self.inner.lock().expect("HttpClient mutex poisoned");
        guard.call(req)
    }
}

#[cfg(test)]
mod test {
    use reqwest::{
        Client as RwClient, Error as RwError, Request as RwRequest, Response as RwResponse,
    };
    use spire_core::BoxError;
    use spire_core::context::{Request, Response};
    use tower::ServiceBuilder;

    use crate::HttpClient;

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
}
