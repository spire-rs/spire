use bytes::Bytes;
use derive_more::{Deref, DerefMut};
use serde::de::DeserializeOwned;

use crate::backend::Client;
use crate::context::Context;
use crate::extract::FromContext;
use crate::{Error, ErrorKind};

/// Bytes [`Response`] body extractor.
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
        let re = cx.resolve().await?;
        let _ = re.into_body();
        todo!()
    }
}

/// Text [`Response`] body extractor.
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
