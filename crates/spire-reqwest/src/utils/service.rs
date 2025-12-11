use spire_core::Error;
use spire_core::context::{Request, Response};
use tower::ServiceBuilder;
use tower::util::BoxCloneService;

use super::conversion::{request_to_reqwest, response_from_reqwest};

/// Type alias for an HTTP service that can handle Spire requests and responses.
///
/// This represents a boxed, cloneable service that takes a [`Request`] and returns
/// a [`Response`], with [`Error`] as the error type. This is the standard service
/// interface used throughout the Spire framework for HTTP operations.
pub type HttpService = BoxCloneService<Request, Response, Error>;

/// Converts a `reqwest::Client` into an [`HttpService`].
///
/// This utility function wraps a reqwest client with the necessary request/response
/// conversions and error mapping to make it compatible with the Spire framework's
/// service interfaces.
///
/// # Examples
///
/// ```no_run
/// use spire_reqwest::client_to_service;
/// use reqwest::Client;
///
/// let client = Client::new();
/// let service = client_to_service(client);
/// ```
pub fn client_to_service(client: reqwest::Client) -> HttpService {
    let svc = ServiceBuilder::default()
        .map_request(request_to_reqwest)
        .map_response(response_from_reqwest)
        .map_err(|x: reqwest::Error| -> Error { Error::from_boxed(x) })
        .service(client);

    BoxCloneService::new(svc)
}

#[cfg(test)]
mod tests {
    use reqwest::Client;

    use super::*;

    #[test]
    fn test_client_to_service() {
        let client = Client::new();
        let _service = client_to_service(client);
        // Test passes if compilation succeeds and function returns HttpService
    }

    #[test]
    fn test_complete_workflow() {
        use crate::HttpClient;

        // Step 1: Create a reqwest client
        let reqwest_client = reqwest::Client::new();

        // Step 2: Convert to HttpService
        let service: HttpService = client_to_service(reqwest_client);

        // Step 3: Create backend from HttpService
        let backend = HttpClient::from_http_service(service.clone());

        // HttpConnection has been removed - HttpClient now implements Client directly

        // Test passes if all conversions work without panicking
        drop(backend);
    }
}
