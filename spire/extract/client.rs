//! [`HttpClient`] extractors.
//!

use std::ops::{Deref, DerefMut};

use scraper::Html as HtmlDoc;

use spire_core::backend::{Client, HttpClient};
use spire_core::context::Context;
use spire_core::Error;
#[cfg(feature = "macros")]
use spire_macros::extract::{Elements, Select};

use crate::extract::{FromContext, Text};

/// TODO.
#[derive(Debug, Clone)]
pub struct Html(pub HtmlDoc);

#[async_trait::async_trait]
impl<C, S> FromContext<C, S> for Html
where
    C: Client,
    S: Sync,
{
    type Rejection = Error;

    async fn from_context(cx: Context<C>, state: &S) -> Result<Self, Self::Rejection> {
        let Text(text) = Text::from_context(cx, state).await?;
        let html = HtmlDoc::parse_document(&text);
        Ok(Html(html))
    }
}

impl Deref for Html {
    type Target = HtmlDoc;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Html {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(feature = "macros")]
#[async_trait::async_trait]
impl<S, T> FromContext<HttpClient, S> for Elements<T>
where
    S: Sync,
    T: Select,
{
    type Rejection = Error;

    async fn from_context(cx: Context<HttpClient>, _state: &S) -> Result<Self, Self::Rejection> {
        // let Html(html) = Html::from_context(cx, state).await?;
        todo!()
    }
}
