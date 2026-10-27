use std::fmt;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use deadpool::Runtime;
use deadpool::managed::Pool;
use spire_core::backend::StatusPolicy;
use spire_core::http::http::Uri;
use spire_core::{Error, Result};
use thirtyfour::{Capabilities, CapabilitiesHelper, ChromiumLikeCapabilities, DesiredCapabilities};

use super::ThirtyfourBackend;
use super::pool::{Endpoint, Sessions};

/// Builds a [`ThirtyfourBackend`].
///
/// At least one endpoint is required: a [`remote`](Self::remote) WebDriver
/// server, or a [`managed`](Self::managed) driver with the `manager` feature.
/// Sessions are spread across endpoints in turn.
#[must_use]
pub struct ThirtyfourBuilder {
    remotes: Vec<String>,
    #[cfg(feature = "manager")]
    managed: bool,
    capabilities: Option<Capabilities>,
    max_sessions: usize,
    acquire_timeout: Option<Duration>,
    page_load_timeout: Duration,
    max_session_age: Option<Duration>,
    isolate: bool,
    status_policy: StatusPolicy,
}

impl Default for ThirtyfourBuilder {
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
            isolate: true,
            status_policy: StatusPolicy::default(),
        }
    }
}

impl ThirtyfourBuilder {
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
    /// use spire_thirtyfour::ThirtyfourBackend;
    /// use spire_thirtyfour::thirtyfour::{ChromiumLikeCapabilities, DesiredCapabilities};
    ///
    /// let mut capabilities = DesiredCapabilities::chrome();
    /// capabilities.add_arg("--headless=new").unwrap();
    /// capabilities.add_arg("--user-agent=MyBot/1.0").unwrap();
    ///
    /// let builder = ThirtyfourBackend::builder().capabilities(capabilities);
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
    /// for a logged-in crawl: requests share the browser's own profile and
    /// tab instead of getting fresh ones.
    pub const fn keep_session_state(mut self) -> Self {
        self.isolate = false;
        self
    }

    /// Sets how error statuses are treated. By default, they become errors
    /// the crawler retries or fails, as for HTTP backends.
    pub const fn status_policy(mut self, policy: StatusPolicy) -> Self {
        self.status_policy = policy;
        self
    }

    /// Builds the backend. No session is started until the first request.
    ///
    /// # Errors
    ///
    /// Returns an [`Invalid`](spire_core::ErrorKind::Invalid) error if no endpoint was
    /// added or a remote URL is not an absolute `http(s)` URL.
    pub fn build(self) -> Result<ThirtyfourBackend> {
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

        let mut capabilities = match self.capabilities {
            Some(capabilities) => capabilities,
            None => headless_chrome()?,
        };
        capabilities.enable_bidi().map_err(|error| {
            Error::invalid("failed to enable WebDriver BiDi").with_source(error)
        })?;
        let sessions = Sessions {
            endpoints,
            next: AtomicUsize::new(0),
            capabilities,
            page_load_timeout: self.page_load_timeout,
            max_age: self.max_session_age,
            isolate: self.isolate,
            status_policy: self.status_policy,
        };

        let pool = Pool::builder(sessions)
            .max_size(self.max_sessions)
            .wait_timeout(self.acquire_timeout)
            .runtime(Runtime::Tokio1)
            .build()
            .map_err(|error| {
                Error::backend("failed to build the session pool").with_source(error)
            })?;
        Ok(ThirtyfourBackend { pool })
    }
}

impl fmt::Debug for ThirtyfourBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThirtyfourBuilder")
            .field("remotes", &self.remotes)
            .field("max_sessions", &self.max_sessions)
            .field("page_load_timeout", &self.page_load_timeout)
            .field("isolate", &self.isolate)
            .field("status_policy", &self.status_policy)
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
