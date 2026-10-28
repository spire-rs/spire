//! Extractors for the [`ThirtyfourSession`] backend.

use std::fmt;
use std::ops::Deref;

use spire_thirtyfour::ThirtyfourSession;
use spire_thirtyfour::thirtyfour::WebDriver;

use crate::extract::FromContext;
#[cfg(feature = "html")]
use crate::extract::html::Html;
use crate::http::http::{HeaderMap, StatusCode};
use crate::{Context, Error, Result};

/// The request's Thirtyfour browser tab for live page interaction.
///
/// Navigates to the request URI if no earlier extractor did.
///
/// Through [`Deref`], `ThirtyfourTab` exposes the full [`WebDriver`] API, pointed at
/// the request's tab, to query and interact with the live page.
/// [`snapshot`](Self::snapshot) parses a copy of the page as currently
/// rendered, including what scripts and interaction changed since it
/// loaded; an [`Html`] argument takes one too, but before the handler runs.
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
/// use spire::extract::{Head, ThirtyfourTab};
/// use spire::frontier::queue::Queue;
///
/// async fn handler(head: Head, queue: Queue, tab: ThirtyfourTab) -> spire::Result<()> {
///     if let Ok(more) = tab.find(By::Css("button.more")).await {
///         let _ = more.click().await;
///     }
///     // The page as it is after the click.
///     let page = tab.snapshot().await?;
///     queue.follow_all(page.links(&head.uri)).await?;
///     Ok(())
/// }
/// # }
/// ```
pub struct ThirtyfourTab {
    driver: WebDriver,
    status: StatusCode,
    headers: HeaderMap,
}

impl ThirtyfourTab {
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

    /// Parses a copy of the page as currently rendered, to search it
    /// locally, e.g. with [`Html::select`]. It does not change with the page:
    /// take another after interacting. Use `find` to query
    /// the live page instead.
    ///
    /// # Errors
    ///
    /// Returns an error if the browser cannot be reached.
    #[cfg(feature = "html")]
    #[cfg_attr(docsrs, doc(cfg(feature = "html")))]
    pub async fn snapshot(&self) -> Result<Html> {
        let source =
            self.driver.source().await.map_err(|error| {
                Error::backend("failed to read the page source").with_source(error)
            })?;
        Ok(Html::parse(&source))
    }
}

impl Deref for ThirtyfourTab {
    type Target = WebDriver;

    fn deref(&self) -> &Self::Target {
        &self.driver
    }
}

impl fmt::Debug for ThirtyfourTab {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThirtyfourTab")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

impl<S: Sync> FromContext<ThirtyfourSession, S> for ThirtyfourTab {
    type Rejection = Error;

    async fn from_context(cx: &Context<ThirtyfourSession>, _: &S) -> Result<Self> {
        let head = cx.response().await?;
        Ok(Self {
            driver: cx.session().driver(),
            status: head.status(),
            headers: head.headers().clone(),
        })
    }
}
