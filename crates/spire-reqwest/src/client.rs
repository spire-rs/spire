use std::fmt;

use spire_core::backend::Client;
use spire_core::context::{Request, Response};
use spire_core::{Error, Result};
use tower::util::BoxCloneService;
use tower::{Service, ServiceExt};

use crate::utils::{request_to_reqwest, response_from_reqwest};

/// HTTP connection that can perform individual HTTP requests.
///
/// `HttpConnection` implements the [`Client`] trait and can use either a reqwest client
/// or a Tower service to perform HTTP requests. Connections are typically created by calling
/// [`HttpClient::connect`].
///
/// [`HttpClient::connect`]: crate::HttpClient::connect
pub struct HttpConnection {
    inner: HttpConnectionInner,
}

enum HttpConnectionInner {
    ReqwestClient(reqwest::Client),
    Service(BoxCloneService<Request, Response, Error>),
}

impl HttpConnection {
    /// Creates a new [`HttpConnection`] from a reqwest client.
    ///
    /// This allows direct use of a reqwest client for HTTP operations.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_reqwest::HttpConnection;
    /// use reqwest::Client;
    ///
    /// let reqwest_client = Client::new();
    /// let connection = HttpConnection::from_reqwest_client(reqwest_client);
    /// ```
    pub fn from_reqwest_client(client: reqwest::Client) -> Self {
        Self {
            inner: HttpConnectionInner::ReqwestClient(client),
        }
    }

    /// Creates a new [`HttpConnection`] from a Tower service.
    ///
    /// This allows wrapping any Tower service that handles HTTP requests.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_reqwest::HttpConnection;
    /// use tower::util::BoxCloneService;
    ///
    /// let service: BoxCloneService<Request, Response, Error> = // ...
    /// let connection = HttpConnection::from_service(service);
    /// ```
    pub fn from_service(service: BoxCloneService<Request, Response, Error>) -> Self {
        Self {
            inner: HttpConnectionInner::Service(service),
        }
    }
}

impl fmt::Debug for HttpConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            HttpConnectionInner::ReqwestClient(_) => f
                .debug_struct("HttpConnection")
                .field("type", &"ReqwestClient")
                .finish_non_exhaustive(),
            HttpConnectionInner::Service(_) => f
                .debug_struct("HttpConnection")
                .field("type", &"Service")
                .finish_non_exhaustive(),
        }
    }
}

#[spire_core::async_trait]
impl Client for HttpConnection {
    /// Resolves an HTTP request using the underlying client implementation.
    ///
    /// Depending on how this connection was created, it will either use a reqwest
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
            HttpConnectionInner::ReqwestClient(client) => {
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
            HttpConnectionInner::Service(mut service) => {
                // Use the Tower service directly
                let ready_service = service.ready().await.map_err(Error::from_boxed)?;
                ready_service.call(req).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_reqwest_client() {
        let client = reqwest::Client::new();
        let connection = HttpConnection::from_reqwest_client(client);
        let debug_str = format!("{:?}", connection);
        assert!(debug_str.contains("HttpConnection"));
        assert!(debug_str.contains("ReqwestClient"));
    }

    #[test]
    fn test_debug_service() {
        use tower::service_fn;
        use tower::util::BoxCloneService;

        let service = service_fn(|_: Request| async { Ok::<Response, Error>(Response::default()) });
        let boxed_service = BoxCloneService::new(service);

        let connection = HttpConnection::from_service(boxed_service);
        let debug_str = format!("{:?}", connection);
        assert!(debug_str.contains("HttpConnection"));
        assert!(debug_str.contains("Service"));
    }

    #[tokio::test]
    async fn test_reqwest_connection_creation() {
        let client = reqwest::Client::new();
        let _connection = HttpConnection::from_reqwest_client(client);
    }
}
