use std::ops::{Deref, DerefMut};

#[cfg(feature = "thirtyfour")]
use spire_thirtyfour::WebDriver;

use crate::Error;
use crate::context::Context;
use crate::extract::{Elements, FromContextRef, Select};

// TODO: Snapshot, Screen, Color, Capture extractors for browser screenshots and visual data.

/// Browser view extractor for direct DOM access.
///
/// Provides access to the rendered page's DOM structure when using the
/// browser automation backend. This is analogous to `Html` for HTTP clients,
/// but works with dynamically rendered content.
///
/// The View wraps a WebDriver instance, allowing direct access to browser
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
#[cfg(feature = "thirtyfour")]
#[derive(Debug)]
pub struct View(pub WebDriver);

#[cfg(not(feature = "thirtyfour"))]
#[derive(Debug, Clone)]
pub struct View(pub ());

#[cfg(feature = "thirtyfour")]
#[async_trait::async_trait]
impl<S> FromContextRef<spire_thirtyfour::BrowserConnection, S> for View
where
    S: Send + Sync + 'static,
{
    type Rejection = Error;

    async fn from_context_ref(
        cx: &Context<spire_thirtyfour::BrowserConnection>,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let client = cx.as_client_ref();
        // Clone the WebDriver from the BrowserConnection
        let driver = (**client).clone();
        Ok(Self(driver))
    }
}

#[cfg(not(feature = "thirtyfour"))]
#[async_trait::async_trait]
impl<C, S> FromContextRef<C, S> for View
where
    C: Send + Sync + 'static,
    S: Send + Sync + 'static,
{
    type Rejection = Error;

    async fn from_context_ref(_cx: &Context<C>, _state: &S) -> Result<Self, Self::Rejection> {
        todo!("View extractor not yet implemented")
    }
}

#[cfg(feature = "thirtyfour")]
impl Deref for View {
    type Target = WebDriver;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(feature = "thirtyfour")]
impl DerefMut for View {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(not(feature = "thirtyfour"))]
impl Deref for View {
    type Target = ();

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(not(feature = "thirtyfour"))]
impl DerefMut for View {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// Remove Clone implementation since WebDriver is not Clone
// Users should extract what they need from the WebDriver instead

#[cfg(all(feature = "macros", feature = "thirtyfour"))]
#[async_trait::async_trait]
impl<S, T> FromContextRef<spire_thirtyfour::BrowserConnection, S> for Elements<T>
where
    S: Sync + Send + 'static,
    T: Select + Send,
{
    type Rejection = Error;

    async fn from_context_ref(
        cx: &Context<spire_thirtyfour::BrowserConnection>,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let View(_view) = View::from_context_ref(cx, state).await?;
        todo!("Elements extractor for BrowserClient not yet implemented")
    }
}
