use std::fmt;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use deadpool::Runtime;
use deadpool::managed::{Pool, PoolError};
use spire_core::backend::Backend;
use spire_core::http::Uri;
use spire_core::{Error, ErrorKind, Result};
use thirtyfour::{Capabilities, ChromiumLikeCapabilities, DesiredCapabilities};

use crate::pool::{Endpoint, Sessions, quit};
use crate::session::BrowserSession;

/// A browser [`Backend`] that hands out pooled WebDriver sessions.
///
/// Sessions are started on demand, up to
/// [`max_sessions`](BrowserBuilder::max_sessions), and reused across
/// requests. Between requests a session's storage and cookies are cleared,
/// unless [`keep_session_state`](BrowserBuilder::keep_session_state) is set.
///
/// Call [`close`](Self::close) once the crawl is over: sessions that are
/// dropped instead of quit block the runtime while they shut down.
///
/// # Examples
///
/// ```no_run
/// use spire_thirtyfour::BrowserBackend;
///
/// # async fn example() -> spire_core::Result<()> {
/// let backend = BrowserBackend::builder()
///     .remote("http://localhost:4444")
///     .max_sessions(4)
///     .build()?;
///
/// // ... crawl ...
///
/// backend.close().await;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct BrowserBackend {
    pool: Pool<Sessions>,
}

impl BrowserBackend {
    /// Returns a builder for a backend.
    #[must_use]
    pub fn builder() -> BrowserBuilder {
        BrowserBuilder::default()
    }

    /// Returns the number of sessions, in use and idle.
    #[must_use]
    pub fn sessions(&self) -> usize {
        self.pool.status().size
    }

    /// Quits every idle session and closes the pool.
    ///
    /// Sessions in use are quit when they would return to the pool, and
    /// further requests fail with a [`Backend`](ErrorKind::Backend) error.
    pub async fn close(&self) {
        let idle = self.pool.retain(|_, _| false).removed;
        for driver in idle {
            quit(driver).await;
        }
        self.pool.close();
    }
}

impl fmt::Debug for BrowserBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = self.pool.status();
        f.debug_struct("BrowserBackend")
            .field("sessions", &status.size)
            .field("idle", &status.available)
            .field("max_sessions", &status.max_size)
            .finish_non_exhaustive()
    }
}

impl Backend for BrowserBackend {
    type Client = BrowserSession;

    async fn connect(&self) -> Result<BrowserSession> {
        let driver = self.pool.get().await.map_err(|error| match error {
            PoolError::Backend(error) => error,
            PoolError::Timeout(_) => Error::new(ErrorKind::Timeout)
                .with_message("timed out waiting for a browser session"),
            PoolError::Closed => Error::backend("browser backend is closed"),
            error => Error::backend("failed to get a browser session").with_source(error),
        })?;

        Ok(BrowserSession::new(driver))
    }
}

/// Builds a [`BrowserBackend`].
///
/// At least one endpoint is required: a [`remote`](Self::remote) WebDriver
/// server, or a [`managed`](Self::managed) driver with the `manager` feature.
/// Sessions are spread across endpoints in turn.
#[must_use]
pub struct BrowserBuilder {
    remotes: Vec<String>,
    #[cfg(feature = "manager")]
    managed: bool,
    capabilities: Option<Capabilities>,
    max_sessions: usize,
    acquire_timeout: Option<Duration>,
    page_load_timeout: Duration,
    max_session_age: Option<Duration>,
    reset: bool,
}

impl Default for BrowserBuilder {
    fn default() -> Self {
        Self {
            remotes: Vec::new(),
            #[cfg(feature = "manager")]
            managed: false,
            capabilities: None,
            max_sessions: 4,
            acquire_timeout: None,
            page_load_timeout: Duration::from_secs(30),
            max_session_age: None,
            reset: true,
        }
    }
}

impl BrowserBuilder {
    /// Adds a WebDriver server run elsewhere, e.g. `chromedriver --port=4444`
    /// or a Selenium grid.
    pub fn remote(mut self, url: impl Into<String>) -> Self {
        self.remotes.push(url.into());
        self
    }

    /// Adds a driver that is downloaded, cached and spawned automatically for
    /// the browser named in the capabilities.
    #[cfg(feature = "manager")]
    #[cfg_attr(docsrs, doc(cfg(feature = "manager")))]
    pub const fn managed(mut self) -> Self {
        self.managed = true;
        self
    }

    /// Sets the capabilities sessions are started with.
    ///
    /// Defaults to headless Chrome. Browser flags such as a user agent are
    /// set here:
    ///
    /// ```
    /// use spire_thirtyfour::BrowserBackend;
    /// use spire_thirtyfour::thirtyfour::{ChromiumLikeCapabilities, DesiredCapabilities};
    ///
    /// let mut capabilities = DesiredCapabilities::chrome();
    /// capabilities.add_arg("--headless=new").unwrap();
    /// capabilities.add_arg("--user-agent=MyBot/1.0").unwrap();
    ///
    /// let builder = BrowserBackend::builder().capabilities(capabilities);
    /// ```
    pub fn capabilities(mut self, capabilities: impl Into<Capabilities>) -> Self {
        self.capabilities = Some(capabilities.into());
        self
    }

    /// Sets the maximum number of sessions, at least one. Defaults to 4.
    pub fn max_sessions(mut self, max: usize) -> Self {
        self.max_sessions = max.max(1);
        self
    }

    /// Fails a request if no session frees up within `timeout`. By default,
    /// requests wait for as long as it takes.
    pub const fn acquire_timeout(mut self, timeout: Duration) -> Self {
        self.acquire_timeout = Some(timeout);
        self
    }

    /// Sets how long a navigation may take. Defaults to 30 seconds.
    pub const fn page_load_timeout(mut self, timeout: Duration) -> Self {
        self.page_load_timeout = timeout;
        self
    }

    /// Replaces sessions once they are older than `age`, e.g. to contain
    /// browser memory growth.
    pub const fn max_session_age(mut self, age: Duration) -> Self {
        self.max_session_age = Some(age);
        self
    }

    /// Keeps storage and cookies between the requests a session serves, e.g.
    /// for a logged-in crawl.
    pub const fn keep_session_state(mut self) -> Self {
        self.reset = false;
        self
    }

    /// Builds the backend. No session is started until the first request.
    ///
    /// # Errors
    ///
    /// Returns an [`Invalid`](ErrorKind::Invalid) error if no endpoint was
    /// added or a remote URL is not an absolute `http(s)` URL.
    pub fn build(self) -> Result<BrowserBackend> {
        let mut endpoints = Vec::with_capacity(self.remotes.len() + 1);
        for url in self.remotes {
            let uri: Uri = url.parse()?;
            if !matches!(uri.scheme_str(), Some("http" | "https")) || uri.host().is_none() {
                return Err(Error::invalid(format!(
                    "WebDriver url `{url}` is not an http(s) url"
                )));
            }
            endpoints.push(Endpoint::Remote(url.trim_end_matches('/').into()));
        }
        #[cfg(feature = "manager")]
        if self.managed {
            let manager = thirtyfour::manager::WebDriverManager::builder().build();
            endpoints.push(Endpoint::Managed(manager));
        }
        if endpoints.is_empty() {
            return Err(Error::invalid(
                "a browser backend needs at least one endpoint",
            ));
        }

        let capabilities = match self.capabilities {
            Some(capabilities) => capabilities,
            None => headless_chrome()?,
        };
        let sessions = Sessions {
            endpoints,
            next: AtomicUsize::new(0),
            capabilities,
            page_load_timeout: self.page_load_timeout,
            max_age: self.max_session_age,
            reset: self.reset,
        };

        let pool = Pool::builder(sessions)
            .max_size(self.max_sessions)
            .wait_timeout(self.acquire_timeout)
            .runtime(Runtime::Tokio1)
            .build()
            .map_err(|error| {
                Error::backend("failed to build the session pool").with_source(error)
            })?;
        Ok(BrowserBackend { pool })
    }
}

impl fmt::Debug for BrowserBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserBuilder")
            .field("remotes", &self.remotes)
            .field("max_sessions", &self.max_sessions)
            .field("page_load_timeout", &self.page_load_timeout)
            .field("reset", &self.reset)
            .finish_non_exhaustive()
    }
}

fn headless_chrome() -> Result<Capabilities> {
    let mut capabilities = DesiredCapabilities::chrome();
    capabilities
        .add_arg("--headless=new")
        .map_err(|error| Error::invalid("invalid default capabilities").with_source(error))?;
    Ok(capabilities.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_an_endpoint() {
        let error = BrowserBackend::builder().build().unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Invalid);
    }

    #[test]
    fn rejects_non_http_remotes() {
        for url in ["localhost:4444", "ftp://localhost", "not a url"] {
            let result = BrowserBackend::builder().remote(url).build();
            assert!(result.is_err(), "{url} should be rejected");
        }
    }

    #[tokio::test]
    async fn builds_lazily_and_closes() {
        let backend = BrowserBackend::builder()
            .remote("http://127.0.0.1:4444/")
            .max_sessions(2)
            .build()
            .unwrap();
        assert_eq!(backend.sessions(), 0);

        backend.close().await;
        let error = backend.connect().await.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Backend);
    }
}
