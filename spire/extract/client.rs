use std::convert::Infallible;

use serde::de::DeserializeOwned;

use spire_core::collect::HandlerContext;

use crate::extract::FromContextParts;

pub struct Body(pub Vec<u8>);

#[async_trait::async_trait]
impl<S> FromContextParts<S> for Body
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &HandlerContext, state: &S) -> Result<Self, Self::Rejection> {
        todo!()
    }
}

pub struct Text(pub String);

#[async_trait::async_trait]
impl<S> FromContextParts<S> for Text
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &HandlerContext, state: &S) -> Result<Self, Self::Rejection> {
        todo!()
    }
}

pub struct Json<T>(pub T);

#[async_trait::async_trait]
impl<S, T> FromContextParts<S> for Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &HandlerContext, state: &S) -> Result<Self, Self::Rejection> {
        todo!()
    }
}

impl<T> From<T> for Json<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}
