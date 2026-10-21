//! Extractors for the [`BrowserSession`] backend.

use std::fmt;
use std::ops::Deref;

use spire_thirtyfour::thirtyfour::WebDriver;
use spire_thirtyfour::{BrowserSession, Navigation};

use crate::extract::FromContext;
#[cfg(feature = "html")]
use crate::extract::Html;
use crate::request::http::{HeaderMap, StatusCode};
use crate::{Context, Error, Result};

/// Navigates the browser to the request URI and hands over the session, to
/// interact with the page.
///
/// It consumes the context, so it is the handler's last argument, in place of
/// a body extractor. Unlike [`Html`], it does not read the page: call
/// [`html`](Self::html) when needed, which also sees what scripts and
/// interaction changed since the page loaded. Through [`Deref`], `View`
/// exposes the full [`WebDriver`] API, pointed at the request's tab.
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
/// use spire::extract::View;
/// use spire::frontier::queue::Queue;
/// use spire::backend::thirtyfour::By;
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
    session: BrowserSession,
    navigation: Navigation,
}

impl View {
    /// Returns the status of the page's response.
    #[must_use]
    pub const fn status(&self) -> StatusCode {
        self.navigation.status()
    }

    /// Returns the headers of the page's response.
    #[must_use]
    pub const fn headers(&self) -> &HeaderMap {
        self.navigation.headers()
    }

    /// Returns a handle to the session's [`WebDriver`], valid while the
    /// handler runs.
    #[must_use]
    pub fn driver(&self) -> WebDriver {
        self.session.driver()
    }

    /// Parses the page as currently rendered.
    ///
    /// # Errors
    ///
    /// Returns an error if the browser cannot be reached.
    #[cfg(feature = "html")]
    #[cfg_attr(docsrs, doc(cfg(feature = "html")))]
    pub async fn html(&self) -> Result<Html> {
        let source = self.session.page_source().await?;
        Ok(Html::parse(&source))
    }

    /// Returns the session, e.g. to hold it past the handler.
    #[must_use]
    pub fn into_session(self) -> BrowserSession {
        self.session
    }
}

impl Deref for View {
    type Target = WebDriver;

    fn deref(&self) -> &Self::Target {
        &self.session
    }
}

impl fmt::Debug for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("View")
            .field("uri", self.navigation.uri())
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}

impl<S: Sync> FromContext<BrowserSession, S> for View {
    type Rejection = Error;

    async fn from_context(cx: Context<BrowserSession>, _: &S) -> Result<Self> {
        let queue = cx.queue().clone();
        let (request, session) = cx.into_parts();
        let navigation = session.navigate(request).await?;
        queue.rebase(navigation.uri().clone());
        Ok(Self {
            session,
            navigation,
        })
    }
}
