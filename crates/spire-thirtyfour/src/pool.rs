//! The pool of WebDriver sessions behind a [`BrowserBackend`].
//!
//! [`BrowserBackend`]: crate::BrowserBackend

use std::fmt;
use std::future::Future;
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
/// tears it down synchronously.
pub(crate) async fn quit(driver: WebDriver) {
    if let Err(error) = driver.quit().await {
        tracing::debug!(target: crate::TRACING_TARGET, %error, "failed to quit session");
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
    type Type = WebDriver;

    async fn create(&self) -> Result<WebDriver> {
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
        Ok(driver)
    }

    async fn recycle(&self, driver: &mut WebDriver, metrics: &Metrics) -> RecycleResult<Error> {
        if self.max_age.is_some_and(|max_age| metrics.age() >= max_age) {
            quit(driver.clone()).await;
            return Err(RecycleError::message("session reached its maximum age"));
        }

        // Listing the profiles also checks the session is alive.
        if let Err(error) = remove_profiles(driver).await {
            quit(driver.clone()).await;
            return Err(RecycleError::Backend(
                error.with_message("session failed to recycle"),
            ));
        }
        Ok(())
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
