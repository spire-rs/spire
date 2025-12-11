use std::fmt;
use std::sync::{Arc, Mutex};

use spire_core::backend::{Backend, Client};
use spire_core::context::{Body, Request, Response};
use spire_core::{Error, ErrorKind, Result};
use tower::util::BoxCloneService;
use tower::{Service, ServiceExt};

use crate::HttpService;
use crate::utils::{request_to_reqwest, response_from_reqwest};

/// HTTP client implementation using reqwest.
///
/// `HttpClient` implements the [`Client`] trait and can perform HTTP requests
/// using the reqwest library or Tower services directly.
///
/// # Examples
///
/// ```ignore
/// use spire_reqwest::HttpClient;
/// use spire_core::backend::Client;
///
/// // Create with default reqwest client
/// let client = HttpClient::default();
///
/// // Create with custom reqwest client
/// let reqwest_client = reqwest::Client::builder()
///     .timeout(std::time::Duration::from_secs(30))
///     .build()
///     .unwrap();
/// let client = HttpClient::from_client(reqwest_client);
///
/// // Create with custom Tower service
/// let client = HttpClient::from_service(my_tower_service);
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
    /// let client = HttpClient::from_client(reqwest_client);
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
    /// let client = HttpClient::from_http_service(service);
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
impl Client for HttpClient {
    /// Resolves an HTTP request using the underlying client implementation.
    ///
    /// Depending on how this client was created, it will either use a reqwest
    /// client or a Tower service to perform the HTTP operation.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The request cannot be processed by the underlying client
    /// - The HTTP request fails (network errors, timeouts, etc.)
    /// - The response cannot be converted back to a spire response
    async fn resolve(self, req: Request) -> Result<Response> {
        match self.inner {
            HttpClientInner::Client(client) => {
                // Convert spire request to reqwest request
                let reqwest_req = request_to_reqwest(req);

                // Perform the HTTP request
                let reqwest_res = client
                    .execute(reqwest_req)
                    .await
                    .map_err(Error::from_boxed)?;

                // Convert reqwest response to spire response
                let response = response_from_reqwest(reqwest_res);

                Ok(response)
            }
            HttpClientInner::Service(service_arc) => {
                // Clone the service from the Arc<Mutex<>>
                let mut service = {
                    let guard = service_arc
                        .lock()
                        .map_err(|_| Error::new(ErrorKind::Backend, "HttpClient mutex poisoned"))?;
                    guard.clone()
                };
                // Use the Tower service directly
                let ready_service = service.ready().await.map_err(Error::from_boxed)?;
                ready_service.call(req).await
            }
        }
    }
}

#[spire_core::async_trait]
impl Backend for HttpClient {
    type Client = Self;

    /// Creates a new HTTP client connection by cloning this client.
    ///
    /// Since HttpClient now implements Client directly, connecting simply
    /// returns a clone of the current client, allowing for efficient reuse.
    async fn connect(&self) -> Result<Self::Client> {
        Ok(self.clone())
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

    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let _client = HttpClient::default();
    }

    #[tokio::test]
    async fn test_from_client() {
        let reqwest_client = reqwest::Client::new();
        let _client = HttpClient::from_client(reqwest_client);
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
        let client = HttpClient::default();
        let debug_str = format!("{:?}", client);
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
