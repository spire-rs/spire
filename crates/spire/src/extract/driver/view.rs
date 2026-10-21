use std::convert::Infallible;

use derive_more::{Deref, DerefMut};
use spire_thirtyfour::BrowserConnection;
use spire_thirtyfour::thirtyfour::WebDriver;

use crate::context::Context;
use crate::extract::FromContextRef;

// TODO: Snapshot, Screen, Color, Capture extractors for browser screenshots and visual data.
// TODO: `Elements` extractor for declarative extraction from rendered pages.

/// Browser view extractor for direct DOM access.
///
/// Provides access to the rendered page's DOM structure when using the
/// browser automation backend. This is analogous to `Html` for HTTP clients,
/// but works with dynamically rendered content.
///
/// The View wraps a `WebDriver` instance, allowing direct access to browser
/// automation methods like `find_element`, `current_url`, `title`, etc.
///
/// # Examples
///
/// ```ignore
/// async fn handler(View(driver): View) -> Result<()> {
///     let title = driver.title().await?;
///     let url = driver.current_url().await?;
///     // ... use WebDriver methods
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct View(pub WebDriver);

#[spire_core::async_trait]
impl<S> FromContextRef<BrowserConnection, S> for View
where
    S: Sync,
{
    type Rejection = Infallible;

    async fn from_context_ref(
        cx: &Context<BrowserConnection>,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let driver: &WebDriver = cx.as_client_ref();
        Ok(Self(driver.clone()))
    }
}
