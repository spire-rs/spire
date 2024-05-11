//! [`BrowserClient`] extractors.
//!

use std::ops::{Deref, DerefMut};

#[cfg(feature = "macros")]
use spire_macros::extract::{Elements, Select};

use crate::backend::{BrowserClient, BrowserPool};
use crate::context::Context;
use crate::extract::FromContextRef;
use crate::Error;

// TODO: Snapshot, Screen, Color, Capture, View.

/// [`Backend`]-specific direct markup extractor.
///
/// [`Backend`]: crate::backend::Backend
#[derive(Debug, Clone)]
pub struct View(pub ());

#[async_trait::async_trait]
impl<S> FromContextRef<BrowserPool, S> for View {
    type Rejection = Error;

    async fn from_context_parts(
        cx: &Context<BrowserPool>,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        todo!()
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

#[cfg(feature = "macros")]
#[async_trait::async_trait]
impl<S, T> FromContextRef<BrowserClient, S> for Elements<T>
where
    S: Sync,
    T: Select,
{
    type Rejection = Error;

    async fn from_context_parts(
        cx: &Context<BrowserClient>,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        todo!()
    }
}
