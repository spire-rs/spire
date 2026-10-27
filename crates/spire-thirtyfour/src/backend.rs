use std::fmt;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use deadpool::Runtime;
use deadpool::managed::{Pool, PoolError};
use spire_core::backend::{Backend, StatusPolicy};
use spire_core::http::Uri;
use spire_core::identity::Identity;
use spire_core::{Error, ErrorKind, Result};
use thirtyfour::bidi::BrowsingContextId;
use thirtyfour::bidi::modules::browsing_context::{Create, CreateType};
use thirtyfour::{
    Capabilities, CapabilitiesHelper, ChromiumLikeCapabilities, DesiredCapabilities, WindowHandle,
};

use crate::error::{from_bidi, from_webdriver};
use crate::pool::{Endpoint, Quit, Sessions};
use crate::session::ThirtyfourSession;

/// A browser [`Backend`] that hands out pooled WebDriver sessions.
///
/// Sessions are started on demand, up to
/// [`max_sessions`](ThirtyfourBuilder::max_sessions), and reused across
/// requests. Every request gets a tab in a fresh browser profile, so no
/// cookies or storage carry over, unless
/// [`keep_session_state`](ThirtyfourBuilder::keep_session_state) is set. With
/// [identities](spire_core::identity), each identity gets a profile of its
/// own instead, see [`connect_as`](Backend::connect_as).
///
/// The backend speaks WebDriver BiDi, which current chromedriver and
/// geckodriver support: responses carry the page's real status and headers,
/// error statuses follow the [`StatusPolicy`], and requests are sent with
/// their method, headers and body. Starting a session fails on a WebDriver
/// server without BiDi.
///
/// Call [`close`](Self::close) once the crawl is over, to quit every session
/// before going on. A backend dropped without it quits its sessions on
/// background tasks instead, which may not finish if the program exits
/// right after.
///
/// # Examples
///
/// ```no_run
/// use spire_thirtyfour::ThirtyfourBackend;
///
/// # async fn example() -> spire_core::Result<()> {
/// let backend = ThirtyfourBackend::builder()
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
pub struct ThirtyfourBackend {
    pool: Pool<Sessions>,
}

impl ThirtyfourBackend {
    /// Returns a builder for a backend.
    pub fn builder() -> ThirtyfourBuilder {
        ThirtyfourBuilder::default()
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
        for mut pooled in idle {
            if let Some(browser) = pooled.take() {
                browser.quit().await;
            }
        }
        self.pool.close();
    }
}

impl fmt::Debug for ThirtyfourBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = self.pool.status();
        f.debug_struct("ThirtyfourBackend")
            .field("sessions", &status.size)
            .field("idle", &status.available)
            .field("max_sessions", &status.max_size)
            .finish_non_exhaustive()
    }
}

impl Backend for ThirtyfourBackend {
    type Client = ThirtyfourSession;

    async fn connect(&self) -> Result<ThirtyfourSession> {
        self.session(None).await
    }

    /// Opens the request's tab in the identity's own browser profile, a
    /// user context with its proxy, user agent and headers, created the
    /// first time the identity uses a browser and kept while it is in use.
    /// Cookies and storage carry over between the identity's requests in
    /// one browser, never to another identity's.
    ///
    /// Proxies asking for credentials are answered with those of the
    /// identity's proxy URL. Browsers do not support SOCKS proxies with
    /// credentials, and fail with an [`Invalid`](ErrorKind::Invalid) error
    /// on drivers without a proxy per user context.
    ///
    /// Identities with user-agent profiles fail as `Invalid` before a
    /// session is acquired: the real browser supplies its own headers.
    async fn connect_as(&self, identity: &Identity) -> Result<ThirtyfourSession> {
        identity.require_no_profile()?;
        self.session(Some(identity)).await
    }
}

impl ThirtyfourBackend {
    /// Hands out a pooled session with a tab for the next request, in the
    /// profile of `identity` if it has one.
    async fn session(&self, identity: Option<&Identity>) -> Result<ThirtyfourSession> {
        // Boxed: the pool's future holds a whole session start.
        let browser = Box::pin(self.pool.get())
            .await
            .map_err(|error| match error {
                PoolError::Backend(error) => error,
                PoolError::Timeout(_) => Error::new(ErrorKind::Timeout)
                    .with_message("timed out waiting for a browser session"),
                PoolError::Closed => Error::backend("browser backend is closed"),
                error => Error::backend("failed to get a browser session").with_source(error),
            })?;
        let sessions = self.pool.manager();
        let bidi = browser.bidi().await.map_err(from_webdriver)?;

        // A tab of its own, in the identity's profile, or in a fresh profile
        // unless state is kept; resetting the browser closes the tab.
        let profile = match identity {
            Some(identity) => Some(browser.profiles.context_of(&bidi, identity).await?),
            None if sessions.isolate => {
                let profile = bidi.browser().create_user_context().await;
                Some(profile.map_err(from_bidi)?.user_context)
            }
            None => None,
        };
        let context = if let Some(profile) = profile {
            let tab = bidi.send(Create {
                r#type: CreateType::Tab,
                reference_context: None,
                background: None,
                user_context: Some(profile),
            });
            let context = tab.await.map_err(from_bidi)?.context;
            if let Some(identity) = identity {
                browser.profiles.opened(&context, identity);
            }
            let handle = WindowHandle::from(context.as_str());
            browser
                .switch_to_window(handle)
                .await
                .map_err(from_webdriver)?;
            context
        } else {
            let handle = browser.window().await.map_err(from_webdriver)?;
            BrowsingContextId::from(handle.to_string())
        };

        Ok(ThirtyfourSession::new(
            browser,
            bidi,
            context,
            sessions.page_load_timeout,
            sessions.status_policy,
        ))
    }
}

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
    /// Returns an [`Invalid`](ErrorKind::Invalid) error if no endpoint was
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_an_endpoint() {
        let error = ThirtyfourBackend::builder().build().unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Invalid);
    }

    #[test]
    fn rejects_non_http_remotes() {
        for url in ["localhost:4444", "ftp://localhost", "not a url"] {
            let result = ThirtyfourBackend::builder().remote(url).build();
            assert!(result.is_err(), "{url} should be rejected");
        }
    }

    #[tokio::test]
    async fn refuses_user_agent_profiles_before_acquiring_a_session() {
        use spire_core::identity::UserAgents;

        let backend = ThirtyfourBackend::builder()
            .remote("http://127.0.0.1:1")
            .build()
            .unwrap();
        let identity = Identity::new("profiled").with_profile(UserAgents::new().pick(0));
        let error = backend.connect_as(&identity).await.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Invalid);
        assert_eq!(backend.sessions(), 0);
        backend.close().await;
    }

    #[tokio::test]
    async fn builds_lazily_and_closes() {
        let backend = ThirtyfourBackend::builder()
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
