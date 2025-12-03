use std::fmt;
use std::sync::{Arc, Mutex};

use spire_core::backend::Backend;
use spire_core::context::{Body, Request, Response};
use spire_core::{Error, ErrorKind, Result};
use tower::util::BoxCloneService;
use tower::{Service, ServiceExt};

use super::connection::HttpConnection;
use crate::HttpService;

/// HTTP backend implementation using reqwest.
///
/// `HttpClient` implements the [`Backend`] trait and creates [`HttpConnection`]
/// instances that can perform HTTP requests using the reqwest library or
/// Tower services.
///
/// # Examples
///
/// ```ignore
/// use spire_reqwest::HttpClient;
/// use spire_core::backend::Backend;
///
/// // Create with default reqwest client
/// let backend = HttpClient::default();
///
/// // Create with custom reqwest client
/// let reqwest_client = reqwest::Client::builder()
///     .timeout(std::time::Duration::from_secs(30))
///     .build()
///     .unwrap();
/// let backend = HttpClient::from_client(reqwest_client);
///
/// // Create with custom Tower service
/// let backend = HttpClient::from_service(my_tower_service);
/// ```
#[derive(Clone)]
pub struct HttpClient {
    inner: HttpClientInner,
}

#[derive(Clone)]
enum HttpClientInner {
    Service(Arc<Mutex<BoxCloneService<Request, Response, Error>>>),
    Client(reqwest::Client),
}

impl HttpClient {
    /// Creates a new [`HttpClient`] from a reqwest client.
    ///
    /// This allows you to customize the underlying reqwest client with
    /// specific configuration like timeouts, headers, proxies, etc.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_reqwest::HttpClient;
    /// use std::time::Duration;
    ///
    /// let reqwest_client = reqwest::Client::builder()
    ///     .timeout(Duration::from_secs(30))
    ///     .user_agent("MyBot/1.0")
    ///     .cookie_store(true)
    ///     .build()
    ///     .unwrap();
    ///
    /// let backend = HttpClient::from_client(reqwest_client);
    /// ```
    pub fn from_client(client: reqwest::Client) -> Self {
        Self {
            inner: HttpClientInner::Client(client),
        }
    }

    /// Creates a new [`HttpClient`] from a Tower service.
    ///
    /// This allows you to wrap any Tower service that can handle HTTP requests
    /// and responses, making it compatible with the Spire backend system.
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
    /// let client = HttpClient::from_service(svc);
    /// ```
    pub fn from_service<S, B, E>(svc: S) -> Self
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

        let inner = HttpClientInner::Service(Arc::new(Mutex::new(BoxCloneService::new(svc))));
        Self { inner }
    }

    /// Creates a new [`HttpClient`] from an [`HttpService`].
    ///
    /// This is a convenience method for creating an HttpClient from the standard
    /// [`HttpService`] type alias used throughout the Spire framework.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_reqwest::{HttpClient, HttpService, client_to_service};
    /// use reqwest::Client;
    ///
    /// let client = Client::new();
    /// let service: HttpService = client_to_service(client);
    /// let backend = HttpClient::from_http_service(service);
    /// ```
    pub fn from_http_service(service: HttpService) -> Self {
        Self {
            inner: HttpClientInner::Service(Arc::new(Mutex::new(service))),
        }
    }

    /// Creates a new [`HttpClient`] with default configuration.
    ///
    /// This creates a basic HTTP client with default reqwest configuration.
    pub fn new() -> Self {
        Self::from_client(reqwest::Client::new())
    }
}

impl Default for HttpClient {
    /// Creates a default HTTP client using a default reqwest client.
    ///
    /// This creates a basic HTTP client with default configuration.
    /// For custom configuration, use [`HttpClient::from_client`] with a configured client.
    fn default() -> Self {
        Self::from_client(reqwest::Client::default())
    }
}

impl fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpClient").finish_non_exhaustive()
    }
}

#[spire_core::async_trait]
impl Backend for HttpClient {
    type Client = HttpConnection;

    /// Creates a new HTTP connection from this backend.
    ///
    /// Each connection gets access to the underlying HTTP client implementation,
    /// allowing for efficient connection reuse and sharing.
    async fn connect(&self) -> Result<Self::Client> {
        match &self.inner {
            HttpClientInner::Client(client) => Ok(HttpConnection::from_client(client.clone())),
            HttpClientInner::Service(service_arc) => {
                let service = {
                    let guard = service_arc
                        .lock()
                        .map_err(|_| Error::new(ErrorKind::Backend, "HttpClient mutex poisoned"))?;
                    guard.clone()
                };
                Ok(HttpConnection::from_service(service))
            }
        }
    }
}

#[cfg(test)]
mod test {
    use reqwest::{
        Client as RwClient, Error as RwError, Request as RwRequest, Response as RwResponse,
    };
    use spire_core::BoxError;
    use spire_core::backend::Backend;
    use spire_core::context::{Request, Response};
    use tower::ServiceBuilder;

    use super::*;

    #[tokio::test]
    async fn test_backend_creation() {
        let backend = HttpClient::default();
        let _connection = backend.connect().await.unwrap();
    }

    #[tokio::test]
    async fn test_from_client() {
        let reqwest_client = reqwest::Client::new();
        let backend = HttpClient::from_client(reqwest_client);
        let _connection = backend.connect().await.unwrap();
    }

    #[test]
    fn test_from_service() {
        // BLOCKED: https://github.com/seanmonstar/reqwest/issues/2039
        // BLOCKED: https://github.com/seanmonstar/reqwest/pull/2060

        let svc = ServiceBuilder::default()
            .map_request(|_x: Request| -> RwRequest { unreachable!() })
            .map_response(|_x: RwResponse| -> Response { unreachable!() })
            .map_err(|x: RwError| -> BoxError { x.into() })
            .service(RwClient::default());

        let _ = HttpClient::from_service(svc);
    }

    #[test]
    fn test_debug() {
        let backend = HttpClient::default();
        let debug_str = format!("{:?}", backend);
        assert!(debug_str.contains("HttpClient"));
    }

    #[test]
    fn test_from_http_service() {
        use crate::{HttpService, client_to_service};

        let reqwest_client = reqwest::Client::new();
        let service: HttpService = client_to_service(reqwest_client);
        let _ = HttpClient::from_http_service(service);
    }
}
