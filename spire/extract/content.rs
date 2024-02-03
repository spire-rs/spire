use std::convert::Infallible;

use serde::de::DeserializeOwned;

use crate::extract::FromContextParts;
use crate::handler::HandlerContext;

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct Text(pub String);

#[async_trait::async_trait]
impl<S> FromContextParts<S> for Text
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &HandlerContext, state: &S) -> Result<Self, Self::Rejection> {
        let Body(body) = Body::from_context_parts(cx, state).await?;

        todo!()
    }
}

// TODO: Html.
#[derive(Debug, Clone)]
pub struct Html(pub ());

#[async_trait::async_trait]
impl<S> FromContextParts<S> for Html
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &HandlerContext, state: &S) -> Result<Self, Self::Rejection> {
        let Body(body) = Body::from_context_parts(cx, state).await?;

        todo!()
    }
}

#[derive(Debug, Clone)]
pub struct Json<T>(pub T);

#[async_trait::async_trait]
impl<S, T> FromContextParts<S> for Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &HandlerContext, state: &S) -> Result<Self, Self::Rejection> {
        let Body(body) = Body::from_context_parts(cx, state).await?;

        todo!()
    }
}

impl<T> From<T> for Json<T> {
    fn from(inner: T) -> Self {
        Self(inner)
    }
}




// pub trait HtmlTransform {
//     type Content;
// }
//
// pub struct Normal;
// impl HtmlTransform for Normal {
//     type Content = u32;
// }
//
// pub struct Reduce;
// impl HtmlTransform for Reduce {
//     type Content = u64;
// }
//
// pub struct Html<T = Normal>(pub T::Content)
//     where
//         T: HtmlTransform;
//
// pub fn handle(Html(body): Html) {}
