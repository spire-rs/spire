use std::future::{Future, ready};

use spire_core::backend::{Backend, Client, StatusPolicy};
use spire_core::request::{Request, Response};
use spire_core::{Error, ErrorKind, Result};

use crate::conversion::{request_to_reqwest, response_from_reqwest};

/// An HTTP [`Backend`] and [`Client`] built on [`reqwest::Client`].
///
/// Every request of a crawl shares the underlying connection pool, cookie
/// store and configuration of the wrapped client. Error statuses are turned
/// into errors according to the [`StatusPolicy`].
///
/// # Examples
///
/// ```
/// use std::time::Duration;
///
/// use spire_reqwest::{HttpClient, StatusPolicy};
///
/// let client = reqwest::Client::builder()
///     .timeout(Duration::from_secs(30))
///     .user_agent("MyBot/1.0")
///     .build()
///     .unwrap();
///
/// let backend = HttpClient::from(client).with_status_policy(StatusPolicy::Accept);
/// ```
#[derive(Debug, Clone, Default)]
pub struct HttpClient {
    client: reqwest::Client,
    status_policy: StatusPolicy,
}

impl HttpClient {
    /// Creates a client with reqwest's default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets how error statuses are treated.
    #[must_use]
    pub const fn with_status_policy(mut self, policy: StatusPolicy) -> Self {
        self.status_policy = policy;
        self
    }

    /// Returns the wrapped [`reqwest::Client`].
    #[must_use]
    pub const fn as_reqwest(&self) -> &reqwest::Client {
        &self.client
    }

    /// Returns the [`StatusPolicy`].
    #[must_use]
    pub const fn status_policy(&self) -> StatusPolicy {
        self.status_policy
    }

    async fn execute(self, request: Request) -> Result<Response> {
        let uri = request.uri().clone();
        let request = request_to_reqwest(request).map_err(|error| error.with_uri(uri.clone()))?;
        let response = self
            .client
            .execute(request)
            .await
            .map_err(|error| classify(error).with_uri(uri.clone()))?;
        self.status_policy.check(response_from_reqwest(response))
    }
}

impl From<reqwest::Client> for HttpClient {
    fn from(client: reqwest::Client) -> Self {
        Self {
            client,
            status_policy: StatusPolicy::default(),
        }
    }
}

impl Backend for HttpClient {
    type Client = Self;

    fn connect(&self) -> impl Future<Output = Result<Self::Client>> + Send {
        ready(Ok(self.clone()))
    }
}

impl Client for HttpClient {
    fn fetch(self, request: Request) -> impl Future<Output = Result<Response>> + Send {
        self.execute(request)
    }
}

/// Classifies a failed transfer, so the crawler only retries what may
/// succeed on another attempt.
fn classify(error: reqwest::Error) -> Error {
    let kind = if error.is_timeout() {
        ErrorKind::Timeout
    } else if error.is_builder() {
        ErrorKind::Invalid
    } else if error.is_redirect() {
        // Too many redirects or a redirect loop; retrying loops again.
        ErrorKind::Rejected
    } else {
        // Connection, DNS, protocol and body errors are usually transient.
        ErrorKind::Http
    };
    Error::new(kind).with_source(error)
}

#[cfg(test)]
mod tests {
    use spire_core::request::Body;

    use super::*;

    #[tokio::test]
    async fn invalid_request_is_rejected_before_sending() {
        let request = Request::new(Body::default());
        let error = HttpClient::new().fetch(request).await.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Invalid);
    }

    #[tokio::test]
    async fn connect_shares_the_client() {
        let backend = HttpClient::new().with_status_policy(StatusPolicy::Accept);
        let client = backend.connect().await.unwrap();
        assert_eq!(client.status_policy(), StatusPolicy::Accept);
    }
}
