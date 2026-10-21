use std::fmt;
use std::ops::Deref;

use deadpool::managed::Object;
use spire_core::backend::Client;
use spire_core::http::header::{CONTENT_TYPE, HeaderValue};
use spire_core::http::{Method, Uri};
use spire_core::request::{Body, Request, Response, ResponseExt};
use spire_core::{Error, Result};
use thirtyfour::WebDriver;

use crate::error::from_webdriver;
use crate::pool::Sessions;

/// A pooled WebDriver session, handed out for one request.
///
/// [`fetch`](Client::fetch) navigates to the request URI and returns the
/// rendered page source; [`navigate`](Self::navigate) only navigates, for
/// handlers that drive the page themselves. Through [`Deref`], the session
/// also exposes the full [`WebDriver`] API for interacting with the page; it
/// returns to the pool when dropped.
///
/// WebDriver cannot report the status or headers of a navigation, so
/// responses are always `200 OK` with an HTML content type, and the request's
/// headers and body are not sent. Only `GET` requests are supported.
pub struct BrowserSession {
    driver: Object<Sessions>,
}

impl BrowserSession {
    pub(crate) const fn new(driver: Object<Sessions>) -> Self {
        Self { driver }
    }

    /// Returns a handle to the session's [`WebDriver`].
    ///
    /// The handle stays valid while the session is in use; once the session
    /// returns to the pool it may be reset or handed to another request.
    #[must_use]
    pub fn driver(&self) -> WebDriver {
        (*self.driver).clone()
    }

    /// Navigates to the request URI without reading the page, returning the
    /// URI the browser ended up on after redirects.
    ///
    /// # Errors
    ///
    /// Returns an [`Invalid`](spire_core::ErrorKind::Invalid) error for
    /// requests other than `GET`, or an error if navigation fails.
    pub async fn navigate(&self, request: &Request) -> Result<Uri> {
        if request.method() != Method::GET {
            let method = request.method();
            return Err(Error::invalid(format!(
                "browser sessions only navigate with GET, not {method}"
            )));
        }

        let uri = request.uri();
        let failed = |error| from_webdriver(error).with_uri(uri.clone());
        self.goto(uri.to_string())
            .await
            .map_err(|error| failed(error).with_message(format!("failed to navigate to {uri}")))?;

        let current = self.current_url().await.map_err(failed)?;
        Ok(current.as_str().parse().unwrap_or_else(|_| uri.clone()))
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
        f.debug_struct("BrowserSession").finish_non_exhaustive()
    }
}

impl Client for BrowserSession {
    async fn fetch(self, request: Request) -> Result<Response> {
        let final_uri = self.navigate(&request).await?;
        let source = self
            .page_source()
            .await
            .map_err(|error| error.with_uri(request.uri().clone()))?;

        let mut response = Response::new(Body::from(source));
        let content_type = HeaderValue::from_static("text/html; charset=utf-8");
        response.headers_mut().insert(CONTENT_TYPE, content_type);
        response.set_final_uri(final_uri);
        Ok(response)
    }
}
