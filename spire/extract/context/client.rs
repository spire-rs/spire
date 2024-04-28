use std::ops::{Deref, DerefMut};

use scraper::Html as HtmlDoc;

use spire_core::backend::{Backend, HttpClient};
use spire_core::context::Context;
use spire_core::Error;
#[cfg(feature = "macros")]
use spire_macros::extract::{Elements, Select};

use crate::extract::{FromContext, FromContextRef, Text};

#[async_trait::async_trait]
impl<S> FromContextRef<HttpClient, S> for HttpClient {
    type Rejection = Error;

    async fn from_context_parts(
        cx: &Context<HttpClient>,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        cx.client().await
    }
}

/// TODO.
#[derive(Debug, Clone)]
pub struct Html(pub HtmlDoc);

#[async_trait::async_trait]
impl<B, S> FromContext<B, S> for Html
where
    B: Backend,
    S: Sync,
{
    type Rejection = Error;

    async fn from_context(cx: Context<B>, state: &S) -> Result<Self, Self::Rejection> {
        let Text(text) = Text::from_context(cx, state).await?;
        let html = HtmlDoc::parse_document(&text);
        Ok(Html(html))
    }
}

impl Deref for Html {
    type Target = HtmlDoc;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Html {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(feature = "macros")]
#[async_trait::async_trait]
impl<S, T> FromContextRef<HttpClient, S> for Elements<T>
where
    S: Sync,
    T: Select,
{
    type Rejection = Error;

    async fn from_context_parts(cx: &Context<HttpClient>, _: &S) -> Result<Self, Self::Rejection> {
        // let Html(html) = Html::from_context(cx, state).await?;
        todo!()
    }
}
