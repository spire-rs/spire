use derive_more::{Deref, DerefMut};
use spire_thirtyfour::BrowserConnection;

use crate::Error;
use crate::context::Context;
use crate::extract::FromContextRef;
#[cfg(feature = "macros")]
use crate::extract::{Elements, Select};

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
#[derive(Debug, Deref, DerefMut)]
pub struct View(pub ());

// #[cfg(feature = "thirtyfour")]
// #[spire_core::async_trait]
// impl<S> FromContextRef<BrowserConnection, S> for View
// where
//     S: Send + Sync + 'static,
// {
//     type Rejection = Error;

//     async fn from_context_ref(cx: &Context<BrowserConnection>, _state: &S) -> Result<Self, Error> {
//         let client = cx.as_client_ref();
//         // Clone the WebDriver from the BrowserConnection
//         let driver = (**client).clone();
//         Ok(Self(driver))
//     }
// }

// Remove Clone implementation since WebDriver is not Clone
// Users should extract what they need from the WebDriver instead

// #[cfg(feature = "macros")]
// #[spire_core::async_trait]
// impl<S, T> FromContextRef<BrowserConnection, S> for Elements<T>
// where
//     S: Sync + Send + 'static,
//     T: Select + Send,
// {
//     type Rejection = Error;

//     async fn from_context_ref(
//         cx: &Context<BrowserConnection>,
//         state: &S,
//     ) -> Result<Self, Self::Rejection> {
//         let View(_view) = View::from_context_ref(cx, state).await?;
//         todo!("Elements extractor for BrowserClient not yet implemented")
//     }
// }
