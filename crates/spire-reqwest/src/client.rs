use std::future::{Future, ready};
use std::sync::Mutex;

use bytes::Bytes;
use spire_core::backend::{Backend, Client, StatusPolicy};
use spire_core::request::{Request, Response};
use spire_core::{Error, ErrorKind, Result};

use crate::conversion::{head_from_reqwest, request_to_reqwest};

/// An HTTP [`Backend`] built on [`reqwest::Client`].
///
/// Every request of a crawl gets an [`HttpSession`], and all of them share
/// the underlying connection pool, cookie store and configuration of the
/// wrapped client. Error statuses are turned into errors according to the
/// [`StatusPolicy`].
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
    type Client = HttpSession;

    fn connect(&self) -> impl Future<Output = Result<Self::Client>> + Send {
        ready(Ok(HttpSession {
            http: self.clone(),
            body: Mutex::new(None),
        }))
    }
}

/// The [`Client`] an [`HttpClient`] hands out for one request: it sends the
/// request and keeps the body it receives.
#[derive(Debug)]
pub struct HttpSession {
    http: HttpClient,
    body: Mutex<Option<Bytes>>,
}

impl HttpSession {
    /// Returns the [`HttpClient`] the session belongs to.
    #[must_use]
    pub const fn http(&self) -> &HttpClient {
        &self.http
    }
}

impl Client for HttpSession {
    async fn fetch(&self, request: &Request) -> Result<Response<()>> {
        let uri = request.uri().clone();
        let failed = |error: Error| error.with_uri(uri.clone());
        let converted = request_to_reqwest(request).map_err(failed)?;
        let response = self
            .http
            .client
            .execute(converted)
            .await
            .map_err(|error| failed(classify(error)))?;

        // An error status is rejected before its body is downloaded.
        let head = self
            .http
            .status_policy
            .check(head_from_reqwest(&response))?;
        let body = response
            .bytes()
            .await
            .map_err(|error| failed(classify(error)))?;
        *lock(&self.body) = Some(body);
        Ok(head)
    }

    fn body(&self) -> impl Future<Output = Result<Bytes>> + Send {
        let body = lock(&self.body).clone();
        ready(body.ok_or_else(|| Error::worker("the request was not fetched yet")))
    }
}

/// Locks a mutex, recovering it if a holder panicked: the body is replaced
/// whole, so it is never left half-written.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
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
    use super::*;

    #[tokio::test]
    async fn invalid_request_is_rejected_before_sending() {
        let session = HttpClient::new().connect().await.unwrap();
        let error = session
            .fetch(&Request::new(Bytes::new()))
            .await
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Invalid);
        assert!(session.body().await.is_err());
    }

    #[tokio::test]
    async fn sessions_share_the_client() {
        let backend = HttpClient::new().with_status_policy(StatusPolicy::Accept);
        let session = backend.connect().await.unwrap();
        assert_eq!(session.http().status_policy(), StatusPolicy::Accept);
    }
}
