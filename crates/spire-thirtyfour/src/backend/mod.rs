mod builder;
pub(crate) mod pool;
mod profiles;

use std::fmt;

use deadpool::managed::{Pool, PoolError};
use spire_core::backend::Backend;
use spire_core::identity::Identity;
use spire_core::{Error, ErrorKind, Result};
use thirtyfour::WindowHandle;
use thirtyfour::bidi::BrowsingContextId;
use thirtyfour::bidi::modules::browsing_context::{Create, CreateType};

pub use self::builder::ThirtyfourBuilder;
use self::pool::{Quit, Sessions};
use crate::error::{from_bidi, from_webdriver};
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
/// error statuses follow the [`StatusPolicy`](crate::StatusPolicy), and requests are sent with
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
    type Session = ThirtyfourSession;

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
