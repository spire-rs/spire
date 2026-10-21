//! Extractors for the [`BrowserSession`] backend.
//!
//! [`BrowserSession`]: spire_thirtyfour::BrowserSession

use std::convert::Infallible;
use std::future::{Future, ready};

use derive_more::{Deref, DerefMut};
use spire_thirtyfour::BrowserSession;
use spire_thirtyfour::thirtyfour::WebDriver;

use crate::Context;
use crate::extract::FromContextRef;

/// Extracts the browser session's [`WebDriver`], to interact with the page.
///
/// `View` does not navigate. Take a body extractor such as
/// [`Html`](crate::extract::Html) as the last argument to load the page
/// first; the driver then shows the rendered page:
///
/// ```no_run
/// use spire::extract::Html;
/// use spire::extract::driver::View;
///
/// async fn handler(View(driver): View, page: Html) -> spire::Result<()> {
///     let rendered = driver.title().await;
///     println!("{rendered:?} / {:?}", page.title());
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct View(pub WebDriver);

impl<S: Sync> FromContextRef<BrowserSession, S> for View {
    type Rejection = Infallible;

    fn from_context_ref(
        cx: &Context<BrowserSession>,
        _: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        ready(Ok(Self(cx.client().driver())))
    }
}
