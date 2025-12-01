//! [`BrowserClient`] extractors.
//!
//! This module provides extractors specifically designed for browser automation
//! using the [`BrowserClient`] backend. These extractors enable interaction with
//! rendered web pages through a WebDriver interface.
//!
//! # Available Extractors
//!
//! - [`View`] - Direct access to the browser's DOM view
//! - [`Elements`] - Declarative extraction of structured data from rendered pages
//!
//! [`BrowserClient`]: spire_thirtyfour::BrowserClient

use std::ops::{Deref, DerefMut};

use crate::context::Context;
use crate::extract::{Elements, FromContextRef, Select};
use crate::Error;

// TODO: Snapshot, Screen, Color, Capture extractors for browser screenshots and visual data.

/// Browser view extractor for direct DOM access.
///
/// Provides access to the rendered page's DOM structure when using the
/// browser automation backend. This is analogous to [`Html`] for HTTP clients,
/// but works with dynamically rendered content.
///
/// # Note
///
/// Currently a placeholder implementation. Full functionality will be added
/// in future versions.
///
/// [`Html`]: super::client::Html
#[derive(Debug, Clone)]
pub struct View(pub ());

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

impl Deref for View {
    type Target = ();

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for View {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(all(feature = "macros", feature = "thirtyfour"))]
#[async_trait::async_trait]
impl<S, T> FromContextRef<spire_thirtyfour::BrowserClient, S> for Elements<T>
where
    S: Sync + Send + 'static,
    T: Select + Send,
{
    type Rejection = Error;

    async fn from_context_ref(
        cx: &Context<spire_thirtyfour::BrowserClient>,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let View(_view) = View::from_context_ref(cx, state).await?;
        todo!("Elements extractor for BrowserClient not yet implemented")
    }
}
