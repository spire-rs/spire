use std::time::Duration;

use ::http::header::{CONTENT_TYPE, IntoHeaderName};
use ::http::{HeaderMap, HeaderValue, StatusCode, Uri};
use bytes::Bytes;

/// A response a [`MockSite`](super::MockSite) serves.
#[derive(Debug, Clone)]
pub struct MockPage {
    pub(super) status: StatusCode,
    pub(super) headers: HeaderMap,
    pub(super) body: Bytes,
    /// Where the page redirects to, if it does.
    pub(super) location: Option<Uri>,
    /// How long the page takes to answer.
    pub(super) latency: Duration,
}

impl MockPage {
    /// Creates an empty page with `status`.
    #[must_use]
    pub fn status(status: StatusCode) -> Self {
        Self {
            status,
            headers: HeaderMap::new(),
            body: Bytes::new(),
            location: None,
            latency: Duration::ZERO,
        }
    }

    /// Creates a `200 OK` HTML page.
    #[must_use]
    pub fn html(body: impl Into<Bytes>) -> Self {
        Self::content("text/html; charset=utf-8", body)
    }

    /// Creates a `200 OK` plain text page.
    #[must_use]
    pub fn text(body: impl Into<Bytes>) -> Self {
        Self::content("text/plain; charset=utf-8", body)
    }

    /// Creates a `200 OK` JSON page.
    #[must_use]
    pub fn json(body: impl Into<Bytes>) -> Self {
        Self::content("application/json", body)
    }

    /// Creates a `200 OK` XML page, e.g. a sitemap.
    #[must_use]
    pub fn xml(body: impl Into<Bytes>) -> Self {
        Self::content("application/xml", body)
    }

    fn content(content_type: &'static str, body: impl Into<Bytes>) -> Self {
        Self::status(StatusCode::OK)
            .with_header(CONTENT_TYPE, HeaderValue::from_static(content_type))
            .with_body(body)
    }

    /// Creates a redirect to `location`, which the site follows as an HTTP
    /// client would: the response is the page at `location`, served from
    /// that URI.
    ///
    /// # Panics
    ///
    /// Panics if `location` is not a valid absolute URI.
    #[must_use]
    pub fn redirect(location: &str) -> Self {
        let mut page = Self::status(StatusCode::FOUND);
        page.location = Some(super::site::parse(location));
        page
    }

    /// Replaces the status.
    #[must_use]
    pub const fn with_status(mut self, status: StatusCode) -> Self {
        self.status = status;
        self
    }

    /// Inserts a header, replacing any previous value.
    #[must_use]
    pub fn with_header(mut self, name: impl IntoHeaderName, value: HeaderValue) -> Self {
        self.headers.insert(name, value);
        self
    }

    /// Takes `latency` to answer, e.g. to test timeouts or adaptive host
    /// delays. Tests with paused time, `#[tokio::test(start_paused = true)]`,
    /// skip the wait.
    #[must_use]
    pub const fn with_latency(mut self, latency: Duration) -> Self {
        self.latency = latency;
        self
    }

    /// Replaces the body.
    #[must_use]
    pub fn with_body(mut self, body: impl Into<Bytes>) -> Self {
        self.body = body.into();
        self
    }
}
