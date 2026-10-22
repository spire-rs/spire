//! The pool of WebDriver sessions behind a [`BrowserBackend`].
//!
//! [`BrowserBackend`]: crate::BrowserBackend

use std::fmt;
use std::future::Future;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use deadpool::managed::{Manager, Metrics, RecycleError, RecycleResult};
use spire_core::backend::StatusPolicy;
use spire_core::{Error, Result};
use thirtyfour::error::WebDriverResult;
use thirtyfour::{Capabilities, WebDriver};

use crate::error::{from_bidi, from_webdriver};

/// The user context every browser starts with; it cannot be removed.
const DEFAULT_USER_CONTEXT: &str = "default";

/// Where sessions are started.
#[derive(Clone)]
pub(crate) enum Endpoint {
    /// A WebDriver server run by the user, e.g. chromedriver or a grid.
    Remote(Arc<str>),
    /// Drivers downloaded and spawned by thirtyfour.
    #[cfg(feature = "manager")]
    Managed(Arc<thirtyfour::manager::WebDriverManager>),
}

/// A boxed future that starts a session.
type Launch<'a> = Pin<Box<dyn Future<Output = WebDriverResult<WebDriver>> + Send + 'a>>;

impl Endpoint {
    /// Starts a session.
    ///
    /// The future is boxed: thirtyfour's session start nests deeply enough
    /// that proving the pool's futures `Send` otherwise exceeds the
    /// compiler's recursion limit.
    fn launch(&self, capabilities: Capabilities) -> Launch<'_> {
        Box::pin(async move {
            match self {
                Self::Remote(url) => WebDriver::new(url.as_ref(), capabilities).await,
                #[cfg(feature = "manager")]
                Self::Managed(manager) => manager.launch(capabilities).await,
            }
        })
    }
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Remote(url) => f.write_str(url),
            #[cfg(feature = "manager")]
            Self::Managed(_) => f.write_str("managed driver"),
        }
    }
}

/// Starts, checks and resets WebDriver sessions for the pool.
pub(crate) struct Sessions {
    pub(crate) endpoints: Vec<Endpoint>,
    pub(crate) next: AtomicUsize,
    /// Capabilities with WebDriver BiDi enabled.
    pub(crate) capabilities: Capabilities,
    pub(crate) page_load_timeout: Duration,
    pub(crate) max_age: Option<Duration>,
    /// Whether every request gets a fresh browser profile.
    pub(crate) isolate: bool,
    pub(crate) status_policy: StatusPolicy,
}

impl Sessions {
    /// Picks the endpoint for the next session, rotating through all of them.
    fn endpoint(&self) -> &Endpoint {
        let index = self.next.fetch_add(1, Ordering::Relaxed) % self.endpoints.len();
        &self.endpoints[index]
    }
}

/// Ends a session that is being discarded.
///
/// Dropping a session without quitting it blocks the runtime while thirtyfour
/// tears it down synchronously; [`Pooled`] quits it on the runtime instead.
pub(crate) async fn quit(driver: WebDriver) {
    if let Err(error) = driver.quit().await {
        tracing::debug!(target: crate::TRACING_TARGET, %error, "failed to quit session");
    }
}

/// Something that must be quit asynchronously before it is dropped.
pub(crate) trait Quit: Send + 'static {
    /// Quits it.
    fn quit(self) -> impl Future<Output = ()> + Send;
}

impl Quit for WebDriver {
    fn quit(self) -> impl Future<Output = ()> + Send {
        quit(self)
    }
}

/// A session in the pool, quit when dropped without blocking the runtime.
///
/// The pool drops sessions it no longer wants, e.g. when the backend is
/// dropped without [`close`](crate::BrowserBackend::close), or a session
/// returns to a closed pool. A dropped session that was not quit yet is
/// quit on a task of the current runtime; only without one does it fall back
/// to thirtyfour's blocking teardown.
pub(crate) struct Pooled<T: Quit = WebDriver>(Option<T>);

impl<T: Quit> Pooled<T> {
    pub(crate) const fn new(value: T) -> Self {
        Self(Some(value))
    }

    /// Takes the session out, to quit it now: it is not quit again on drop.
    pub(crate) const fn take(&mut self) -> Option<T> {
        self.0.take()
    }
}

impl<T: Quit> Deref for Pooled<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.0
            .as_ref()
            .unwrap_or_else(|| unreachable!("a taken session is never used again"))
    }
}

impl<T: Quit> Drop for Pooled<T> {
    fn drop(&mut self) {
        if let Some(value) = self.0.take()
            && let Ok(runtime) = tokio::runtime::Handle::try_current()
        {
            runtime.spawn(value.quit());
        }
    }
}

/// Removes the browser profiles the session's requests used, closing their
/// tabs, cookies and storage with them.
async fn remove_profiles(driver: &WebDriver) -> Result<()> {
    let bidi = driver.bidi().await.map_err(from_webdriver)?;
    let browser = bidi.browser();
    let profiles = browser.get_user_contexts().await.map_err(from_bidi)?;
    for profile in profiles.user_contexts {
        if profile.user_context.as_str() != DEFAULT_USER_CONTEXT {
            browser
                .remove_user_context(profile.user_context)
                .await
                .map_err(from_bidi)?;
        }
    }
    Ok(())
}

impl Manager for Sessions {
    type Error = Error;
    type Type = Pooled;

    async fn create(&self) -> Result<Pooled> {
        let endpoint = self.endpoint();
        let driver = endpoint
            .launch(self.capabilities.clone())
            .await
            .map_err(|error| {
                from_webdriver(error)
                    .with_message(format!("failed to start a session on {endpoint}"))
            })?;

        if let Err(error) = driver.bidi().await {
            quit(driver).await;
            let message = format!("{endpoint} does not support WebDriver BiDi");
            return Err(from_webdriver(error).with_message(message));
        }
        if let Err(error) = driver.set_page_load_timeout(self.page_load_timeout).await {
            quit(driver).await;
            return Err(from_webdriver(error).with_message("failed to set the page load timeout"));
        }

        tracing::debug!(target: crate::TRACING_TARGET, %endpoint, "session started");
        Ok(Pooled::new(driver))
    }

    async fn recycle(&self, pooled: &mut Pooled, metrics: &Metrics) -> RecycleResult<Error> {
        let expired = self.max_age.is_some_and(|max_age| metrics.age() >= max_age);
        // Listing the profiles also checks the session is alive.
        let failed = if expired {
            None
        } else {
            remove_profiles(pooled).await.err()
        };
        if !expired && failed.is_none() {
            return Ok(());
        }

        if let Some(driver) = pooled.take() {
            quit(driver).await;
        }
        Err(match failed {
            Some(error) => RecycleError::Backend(error.with_message("session failed to recycle")),
            None => RecycleError::message("session reached its maximum age"),
        })
    }
}

impl fmt::Debug for Sessions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let endpoints: Vec<_> = self.endpoints.iter().map(ToString::to_string).collect();
        f.debug_struct("Sessions")
            .field("endpoints", &endpoints)
            .field("page_load_timeout", &self.page_load_timeout)
            .field("max_age", &self.max_age)
            .field("isolate", &self.isolate)
            .field("status_policy", &self.status_policy)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use super::*;

    /// A session that records being quit.
    struct Fake(Arc<AtomicBool>);

    impl Quit for Fake {
        fn quit(self) -> impl Future<Output = ()> + Send {
            self.0.store(true, Ordering::SeqCst);
            std::future::ready(())
        }
    }

    #[tokio::test]
    async fn dropped_sessions_are_quit_on_the_runtime() {
        let quit = Arc::new(AtomicBool::new(false));
        drop(Pooled::new(Fake(Arc::clone(&quit))));
        // Quit on a task, not during the drop.
        tokio::task::yield_now().await;
        assert!(quit.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn taken_sessions_are_not_quit_again() {
        let quit = Arc::new(AtomicBool::new(false));
        let mut pooled = Pooled::new(Fake(Arc::clone(&quit)));
        assert!(pooled.take().is_some());
        drop(pooled);
        tokio::task::yield_now().await;
        assert!(!quit.load(Ordering::SeqCst));
    }

    #[test]
    fn without_a_runtime_dropping_does_not_panic() {
        let quit = Arc::new(AtomicBool::new(false));
        drop(Pooled::new(Fake(Arc::clone(&quit))));
        assert!(!quit.load(Ordering::SeqCst));
    }
}
