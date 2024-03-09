use std::convert::Infallible;

use bytes::Bytes;
use serde::de::DeserializeOwned;

use spire_core::backend::Backend;
use spire_core::context::{Context, Response};

use crate::extract::FromContext;

/// TODO.
#[derive(Debug, Clone)]
pub struct Body(pub Bytes);

#[async_trait::async_trait]
impl<B, S> FromContext<B, S> for Body
where
    B: Backend,
    S: Sync,
{
    type Rejection = Infallible;

    async fn from_context(cx: Context<B>, state: &S) -> Result<Self, Self::Rejection> {
        let _ = Response::from_context(cx, state).await;
        todo!()
    }
}

/// TODO.
#[derive(Debug, Clone)]
pub struct Text(pub String);

#[async_trait::async_trait]
impl<B, S> FromContext<B, S> for Text
where
    B: Backend,
    S: Sync,
{
    type Rejection = Infallible;

    async fn from_context(cx: Context<B>, state: &S) -> Result<Self, Self::Rejection> {
        let _ = Body::from_context(cx, state).await;
        todo!()
    }
}

/// TODO.
/// Mostly used for API scraping.
#[derive(Debug, Default, Clone, Copy)]
pub struct Json<T>(pub T);

#[async_trait::async_trait]
impl<B, S, T> FromContext<B, S> for Json<T>
where
    B: Backend,
    S: Sync,
    T: DeserializeOwned,
{
    type Rejection = Infallible;

    async fn from_context(cx: Context<B>, state: &S) -> Result<Self, Self::Rejection> {
        let _ = Body::from_context(cx, state).await;
        // serde_json::from_slice()
        todo!()
    }
}
