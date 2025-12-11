use std::convert::Infallible;

use bytes::Bytes;
use derive_more::{Deref, DerefMut};
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;

use crate::backend::Client;
use crate::context::{Context, Depth, TaskExt};
use crate::extract::{FromContext, FromContextRef};
use crate::{Error, ErrorKind};

/// Bytes [`Response`] body extractor.
///
/// ⚠️ Since parsing bytes requires consuming the response body,
/// the Body extractor must be last if there are multiple extractors
/// in a handler.
///
/// [`Response`]: crate::context::Response
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct Body(pub Bytes);

#[spire_core::async_trait]
impl<C, S> FromContext<C, S> for Body
where
    C: Client,
    S: Sync,
{
    type Rejection = Error;

    async fn from_context(cx: Context<C>, _state: &S) -> Result<Self, Self::Rejection> {
        let response = cx.resolve().await?;
        let body = response.into_body();
        let bytes = body
            .collect()
            .await
            .map_err(|e| Error::with_source(ErrorKind::Context, "failed to read response body", e))?
            .to_bytes();
        Ok(Self(bytes))
    }
}

/// Text [`Response`] body extractor.
///
/// ⚠️ Since parsing text requires consuming the response body,
/// the Text extractor must be last if there are multiple extractors
/// in a handler.
///
/// [`Response`]: crate::context::Response
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct Text(pub String);

#[spire_core::async_trait]
impl<C, S> FromContext<C, S> for Text
where
    C: Client,
    S: Sync,
{
    type Rejection = Error;

    async fn from_context(cx: Context<C>, state: &S) -> Result<Self, Self::Rejection> {
        let Body(bytes) = Body::from_context(cx, state).await?;
        String::from_utf8(bytes.to_vec()).map(Self).map_err(|e| {
            Error::with_source(ErrorKind::Context, "failed to parse UTF-8", Box::new(e))
        })
    }
}

/// Parsed HTML document extractor.
///
/// Extracts the response body as HTML text. This is a dummy implementation that
/// only stores the raw HTML string without parsing capabilities.
///
/// ⚠️ Since parsing HTML requires consuming the response body,
/// the Html extractor must be last if there are multiple extractors
/// in a handler.
///
/// # Examples
///
/// ```ignore
/// use spire::extract::Html;
///
/// async fn handler(Html(html): Html) {
///     println!("HTML content: {}", html.as_str());
/// }
/// ```
///
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct Html(pub String);

impl Html {
    /// Get a reference to the raw HTML string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert into the raw HTML string.
    pub fn into_string(self) -> String {
        self.0
    }
}

#[spire_core::async_trait]
impl<C, S> FromContext<C, S> for Html
where
    C: Client,
    S: Sync,
{
    type Rejection = Error;

    async fn from_context(cx: Context<C>, state: &S) -> Result<Self, Self::Rejection> {
        let Text(text) = Text::from_context(cx, state).await?;
        Ok(Self(text))
    }
}

/// JSON [`Response`] body extractor.
///
/// Useful for the API scraping.
///
/// ⚠️ Since parsing JSON requires consuming the response body,
/// the Json extractor must be last if there are multiple extractors
/// in a handler.
///
/// [`Response`]: crate::context::Response
#[derive(Debug, Default, Clone, Copy, Deref, DerefMut)]
pub struct Json<T>(pub T);

#[spire_core::async_trait]
impl<C, S, T> FromContext<C, S> for Json<T>
where
    C: Client,
    S: Sync,
    T: DeserializeOwned,
{
    type Rejection = Error;

    async fn from_context(cx: Context<C>, state: &S) -> Result<Self, Self::Rejection> {
        let Body(bytes) = Body::from_context(cx, state).await?;
        serde_json::from_slice::<T>(&bytes).map(Self).map_err(|e| {
            Error::with_source(ErrorKind::Context, "failed to parse JSON", Box::new(e))
        })
    }
}

/// Depth extractor for tracking request recursion depth.
#[spire_core::async_trait]
impl<C, S> FromContextRef<C, S> for Depth
where
    C: Sync,
    S: Sync,
{
    type Rejection = Infallible;

    async fn from_context_ref(cx: &Context<C>, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Depth::new(cx.get_ref().depth()))
    }
}
