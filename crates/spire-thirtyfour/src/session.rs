use std::fmt;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bytes::Bytes;
use deadpool::managed::Object;
use spire_core::backend::{Client, StatusPolicy};
use spire_core::http::header::{
    CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, HeaderValue, TRANSFER_ENCODING,
};
use spire_core::request::{Request, Response, ResponseExt};
use spire_core::{Error, Result};
use thirtyfour::WebDriver;
use thirtyfour::bidi::{BiDi, BrowsingContextId};

use crate::error::from_webdriver;
use crate::navigate::{Overrides, navigate};
use crate::pool::Sessions;

/// A pooled WebDriver session, handed out for one request.
///
/// Each request gets a tab of its own, in a fresh browser profile unless the
/// backend keeps session state, so cookies and storage do not leak between
/// requests. [`fetch`](Client::fetch) navigates the tab to the request URI,
/// and [`body`](Client::body) returns the page as currently rendered, so it
/// reflects whatever scripts and interaction changed since it loaded.
/// Through [`Deref`], the session also exposes the full [`WebDriver`] API,
/// pointed at the request's tab; it returns to the pool when dropped.
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
    /// Whether a navigation reached a page, so there is a body to read.
    navigated: AtomicBool,
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
            navigated: AtomicBool::new(false),
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
    /// Navigates the tab to the request URI, sent with its method, headers
    /// and body, and returns the head of the response the page ended on
    /// after redirects, checked against the backend's [`StatusPolicy`].
    ///
    /// The head has the page's status and headers, but the body is the
    /// rendered document serialized as HTML, so its content type is always
    /// `text/html; charset=utf-8`, whatever the server sent.
    async fn fetch(&self, request: &Request) -> Result<Response<()>> {
        let overrides = Overrides {
            method: request.method().clone(),
            headers: request.headers().clone(),
            body: request.body().to_vec(),
        };
        let navigation = navigate(
            &self.bidi,
            &self.context,
            request.uri(),
            &overrides,
            self.page_load_timeout,
        )
        .await?;

        let mut head = Response::new(());
        *head.status_mut() = navigation.status();
        let headers = head.headers_mut();
        *headers = navigation.headers().clone();
        for header in [CONTENT_LENGTH, CONTENT_ENCODING, TRANSFER_ENCODING] {
            headers.remove(header);
        }
        let content_type = HeaderValue::from_static("text/html; charset=utf-8");
        headers.insert(CONTENT_TYPE, content_type);
        head.set_final_uri(navigation.uri().clone());
        self.navigated.store(true, Ordering::Release);
        self.status_policy.check(head)
    }

    /// Returns the page as currently rendered.
    async fn body(&self) -> Result<Bytes> {
        if !self.navigated.load(Ordering::Acquire) {
            return Err(Error::worker("the request was not fetched yet"));
        }
        Ok(Bytes::from(self.page_source().await?))
    }
}
