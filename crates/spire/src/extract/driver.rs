//! Extractors for the [`BrowserSession`] backend.

use std::fmt;
use std::ops::Deref;

use spire_thirtyfour::BrowserSession;
use spire_thirtyfour::thirtyfour::WebDriver;

use crate::extract::FromContext;
#[cfg(feature = "html")]
use crate::extract::Html;
use crate::request::http::{HeaderMap, StatusCode};
use crate::{Context, Error, Result};

/// Navigates the browser to the request URI, if no extractor did yet, and
/// hands over the browser, to interact with the page.
///
/// Through [`Deref`], `View` exposes the full [`WebDriver`] API, pointed at
/// the request's tab. [`html`](Self::html) reads the page as currently
/// rendered, including what scripts and interaction changed since it
/// loaded; an [`Html`] argument reads it too, but before the handler runs.
/// Relative links followed through the queue resolve against the URI the
/// browser ended up on.
///
/// Error statuses are rejected by the backend's status policy before the
/// handler runs; [`status`](Self::status) and [`headers`](Self::headers)
/// describe the page's response.
///
/// ```no_run
/// # #[cfg(feature = "html")]
/// # mod example {
/// use spire::backend::thirtyfour::By;
/// use spire::extract::View;
/// use spire::frontier::queue::Queue;
///
/// async fn handler(queue: Queue, view: View) -> spire::Result<()> {
///     if let Ok(more) = view.find(By::Css("button.more")).await {
///         let _ = more.click().await;
///     }
///     let page = view.html().await?;
///     queue.follow_all(page.links()).await?;
///     Ok(())
/// }
/// # }
/// ```
pub struct View {
    driver: WebDriver,
    status: StatusCode,
    headers: HeaderMap,
}

impl View {
    /// Returns the status of the page's response.
    #[must_use]
    pub const fn status(&self) -> StatusCode {
        self.status
    }

    /// Returns the headers of the page's response.
    #[must_use]
    pub const fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Returns a handle to the browser, valid while the request is handled.
    #[must_use]
    pub fn driver(&self) -> WebDriver {
        self.driver.clone()
    }

    /// Parses the page as currently rendered.
    ///
    /// # Errors
    ///
    /// Returns an error if the browser cannot be reached.
    #[cfg(feature = "html")]
    #[cfg_attr(docsrs, doc(cfg(feature = "html")))]
    pub async fn html(&self) -> Result<Html> {
        let source =
            self.driver.source().await.map_err(|error| {
                Error::backend("failed to read the page source").with_source(error)
            })?;
        Ok(Html::parse(&source))
    }
}

impl Deref for View {
    type Target = WebDriver;

    fn deref(&self) -> &Self::Target {
        &self.driver
    }
}

impl fmt::Debug for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("View")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

impl<S: Sync> FromContext<BrowserSession, S> for View {
    type Rejection = Error;

    async fn from_context(cx: &Context<BrowserSession>, _: &S) -> Result<Self> {
        let head = cx.response().await?;
        Ok(Self {
            driver: cx.client().driver(),
            status: head.status(),
            headers: head.headers().clone(),
        })
    }
}
