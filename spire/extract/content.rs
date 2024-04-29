use bytes::Bytes;
use serde::de::DeserializeOwned;

use spire_core::backend::Client;
use spire_core::context::Context;
use spire_core::Error;

use crate::extract::FromContext;

/// TODO.
#[derive(Debug, Clone)]
pub struct Body(pub Bytes);

#[async_trait::async_trait]
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

/// Text [`Response`] extractor.
///
/// [`Response`]: crate::context::Response
#[derive(Debug, Clone)]
pub struct Text(pub String);

#[async_trait::async_trait]
impl<C, S> FromContext<C, S> for Text
where
    C: Client,
    S: Sync,
{
    type Rejection = Error;

    async fn from_context(cx: Context<C>, state: &S) -> Result<Self, Self::Rejection> {
        let Body(bytes) = Body::from_context(cx, state).await?;
        let inner = String::from_utf8(bytes.to_vec()).map_err(Error::new)?;
        Ok(Text(inner))
    }
}

/// JSON [`Response`] extractor.
///
/// Useful for API scraping.
///
/// [`Response`]: crate::context::Response
#[derive(Debug, Default, Clone, Copy)]
pub struct Json<T>(pub T);

#[async_trait::async_trait]
impl<C, S, T> FromContext<C, S> for Json<T>
where
    C: Client,
    S: Sync,
    T: DeserializeOwned,
{
    type Rejection = Error;

    async fn from_context(cx: Context<C>, state: &S) -> Result<Self, Self::Rejection> {
        let Body(bytes) = Body::from_context(cx, state).await?;
        let inner = serde_json::from_slice::<T>(bytes.as_ref()).map_err(Error::new)?;
        Ok(Json(inner))
    }
}
