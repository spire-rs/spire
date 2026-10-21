//! The pool of WebDriver sessions behind a [`BrowserBackend`].
//!
//! [`BrowserBackend`]: crate::BrowserBackend

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use deadpool::managed::{Manager, Metrics, RecycleError, RecycleResult};
use spire_core::{Error, Result};
use thirtyfour::error::WebDriverResult;
use thirtyfour::{Capabilities, WebDriver};

use crate::error::from_webdriver;

/// Clears what a page leaves in a session: storage, cookies and the page.
const RESET_STORAGE: &str =
    "try { window.localStorage.clear(); window.sessionStorage.clear(); } catch (_) {}";

/// Where sessions are started.
#[derive(Clone)]
pub(crate) enum Endpoint {
    /// A WebDriver server run by the user, e.g. chromedriver or a grid.
    Remote(Arc<str>),
    /// Drivers downloaded and spawned by thirtyfour.
    #[cfg(feature = "manager")]
    Managed(Arc<thirtyfour::manager::WebDriverManager>),
}

impl Endpoint {
    async fn launch(&self, capabilities: Capabilities) -> WebDriverResult<WebDriver> {
        match self {
            Self::Remote(url) => WebDriver::new(url.as_ref(), capabilities).await,
            #[cfg(feature = "manager")]
            Self::Managed(manager) => manager.launch(capabilities).await,
        }
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
    pub(crate) capabilities: Capabilities,
    pub(crate) page_load_timeout: Duration,
    pub(crate) max_age: Option<Duration>,
    pub(crate) reset: bool,
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

async fn reset(driver: &WebDriver) -> WebDriverResult<()> {
    driver.execute(RESET_STORAGE, Vec::new()).await?;
    driver.delete_all_cookies().await?;
    driver.goto("about:blank").await
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

        let checked = if self.reset {
            reset(driver).await
        } else {
            driver.current_url().await.map(drop)
        };

        if let Err(error) = checked {
            quit(driver.clone()).await;
            return Err(RecycleError::Backend(
                from_webdriver(error).with_message("session failed to recycle"),
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
            .field("reset", &self.reset)
            .finish_non_exhaustive()
    }
}
