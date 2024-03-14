use bytes::Bytes;
use serde::de::DeserializeOwned;
use tower::Service;

use spire_core::context::{Context, Request, Response};
use spire_core::Error;

use crate::extract::FromContext;

/// TODO.
#[derive(Debug, Clone)]
pub struct Body(pub Bytes);

#[async_trait::async_trait]
impl<B, S> FromContext<B, S> for Body
where
    B: Service<Request, Response = Response, Error = Error> + Send + Sync + 'static,
    <B as Service<Request>>::Future: Send,
    S: Sync,
{
    type Rejection = Error;

    async fn from_context(cx: Context<B>, _state: &S) -> Result<Self, Self::Rejection> {
        let _ = cx.try_resolve().await?;
        todo!()
    }
}

/// TODO.
#[derive(Debug, Clone)]
pub struct Text(pub String);

#[async_trait::async_trait]
impl<B, S> FromContext<B, S> for Text
where
    B: Service<Request, Response = Response, Error = Error> + Send + Sync + 'static,
    <B as Service<Request>>::Future: Send,
    S: Sync,
{
    type Rejection = Error;

    async fn from_context(cx: Context<B>, state: &S) -> Result<Self, Self::Rejection> {
        let Body(bytes) = Body::from_context(cx, state).await?;
        let inner = String::from_utf8(bytes.to_vec()).map_err(Error::new)?;
        Ok(Text(inner))
    }
}

/// TODO.
/// Mostly used for API scraping.
#[derive(Debug, Default, Clone, Copy)]
pub struct Json<T>(pub T);

#[async_trait::async_trait]
impl<B, S, T> FromContext<B, S> for Json<T>
where
    B: Service<Request, Response = Response, Error = Error> + Send + Sync + 'static,
    <B as Service<Request>>::Future: Send,
    S: Sync,
    T: DeserializeOwned,
{
    type Rejection = Error;

    async fn from_context(cx: Context<B>, state: &S) -> Result<Self, Self::Rejection> {
        let Body(bytes) = Body::from_context(cx, state).await?;
        let inner = serde_json::from_slice::<T>(bytes.as_ref()).map_err(Error::new)?;
        Ok(Json(inner))
    }
}

/// TODO.
#[derive(Debug, Clone)]
pub struct Html(pub ());

#[async_trait::async_trait]
impl<B, S> FromContext<B, S> for Html
where
    B: Service<Request, Response = Response, Error = Error> + Send + Sync + 'static,
    <B as Service<Request>>::Future: Send,
    S: Sync,
{
    type Rejection = Error;

    async fn from_context(cx: Context<B>, state: &S) -> Result<Self, Self::Rejection> {
        let Body(bytes) = Body::from_context(cx, state).await?;
        todo!()
    }
}
