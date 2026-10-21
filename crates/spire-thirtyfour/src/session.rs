use std::fmt;
use std::ops::Deref;
use std::time::Duration;

use deadpool::managed::Object;
use http_body_util::BodyExt;
use spire_core::backend::{Client, StatusPolicy};
use spire_core::http::header::{
    CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, HeaderValue, TRANSFER_ENCODING,
};
use spire_core::request::{Body, Request, Response, ResponseExt};
use spire_core::{Error, Result};
use thirtyfour::WebDriver;
use thirtyfour::bidi::{BiDi, BrowsingContextId};

use crate::error::from_webdriver;
use crate::navigate::{Navigation, Overrides, navigate};
use crate::pool::Sessions;

/// A pooled WebDriver session, handed out for one request.
///
/// Each request gets a tab of its own, in a fresh browser profile unless the
/// backend keeps session state, so cookies and storage do not leak between
/// requests. [`fetch`](Client::fetch) navigates the tab to the request URI
/// and returns the rendered page; [`navigate`](Self::navigate) only
/// navigates, for handlers that drive the page themselves. Through
/// [`Deref`], the session also exposes the full [`WebDriver`] API, pointed
/// at the request's tab; it returns to the pool when dropped.
///
/// Navigation goes through WebDriver BiDi, so responses carry the page's
/// real status and headers, and requests are sent with their method,
/// headers and body.
pub struct BrowserSession {
    driver: Object<Sessions>,
    bidi: BiDi,
    context: BrowsingContextId,
    page_load_timeout: Duration,
    status_policy: StatusPolicy,
}

impl BrowserSession {
    pub(crate) const fn new(
        driver: Object<Sessions>,
        bidi: BiDi,
        context: BrowsingContextId,
        page_load_timeout: Duration,
        status_policy: StatusPolicy,
    ) -> Self {
        Self {
            driver,
            bidi,
            context,
            page_load_timeout,
            status_policy,
        }
    }

    /// Returns a handle to the session's [`WebDriver`].
    ///
    /// The handle stays valid while the session is in use; once the session
    /// returns to the pool it may be reset or handed to another request.
    #[must_use]
    pub fn driver(&self) -> WebDriver {
        (*self.driver).clone()
    }

    /// Returns the session's WebDriver BiDi connection, e.g. to listen to
    /// console logs or network events.
    #[must_use]
    pub const fn bidi(&self) -> &BiDi {
        &self.bidi
    }

    /// Returns the browsing context, the tab, the request is handled in.
    #[must_use]
    pub const fn context(&self) -> &BrowsingContextId {
        &self.context
    }

    /// Navigates to the request URI without reading the page, returning the
    /// response the page ended on after redirects.
    ///
    /// The request is sent with its method, headers and body. Its status is
    /// checked against the backend's [`StatusPolicy`].
    ///
    /// # Errors
    ///
    /// Returns an error if the navigation fails or times out, or if the
    /// status is an error under the [`StatusPolicy`].
    pub async fn navigate(&self, request: Request) -> Result<Navigation> {
        let (parts, body) = request.into_parts();
        let body = body.collect().await.map_err(|error| {
            Error::invalid("failed to read the request body")
                .with_source(error)
                .with_uri(parts.uri.clone())
        })?;
        let overrides = Overrides {
            method: parts.method,
            headers: parts.headers,
            body: body.to_bytes().to_vec(),
        };

        let navigation = navigate(
            &self.bidi,
            &self.context,
            &parts.uri,
            &overrides,
            self.page_load_timeout,
        )
        .await?;

        let mut response = Response::new(());
        *response.status_mut() = navigation.status();
        *response.headers_mut() = navigation.headers().clone();
        response.set_final_uri(navigation.uri().clone());
        self.status_policy.check(response)?;
        Ok(navigation)
    }

    /// Returns the source of the page as currently rendered, including
    /// changes made by scripts and interaction since it loaded.
    ///
    /// # Errors
    ///
    /// Returns an error if the browser cannot be reached.
    pub async fn page_source(&self) -> Result<String> {
        self.source().await.map_err(from_webdriver)
    }
}

impl Deref for BrowserSession {
    type Target = WebDriver;

    fn deref(&self) -> &Self::Target {
        &self.driver
    }
}

impl fmt::Debug for BrowserSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserSession")
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

impl Client for BrowserSession {
    /// Navigates to the request URI and returns the rendered page.
    ///
    /// The response has the page's status and headers, but its body is the
    /// rendered document serialized as HTML, so it is always
    /// `text/html; charset=utf-8`, whatever the server sent.
    async fn fetch(self, request: Request) -> Result<Response> {
        let uri = request.uri().clone();
        let navigation = self.navigate(request).await?;
        let source = self
            .page_source()
            .await
            .map_err(|error| error.with_uri(uri))?;

        let mut response = Response::new(Body::from(source));
        *response.status_mut() = navigation.status();
        let headers = response.headers_mut();
        *headers = navigation.headers().clone();
        for header in [CONTENT_LENGTH, CONTENT_ENCODING, TRANSFER_ENCODING] {
            headers.remove(header);
        }
        let content_type = HeaderValue::from_static("text/html; charset=utf-8");
        headers.insert(CONTENT_TYPE, content_type);
        response.set_final_uri(navigation.uri().clone());
        Ok(response)
    }
}
