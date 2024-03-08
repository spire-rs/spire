// TODO: Context: Backend, TaskQueue, Dataset.
// TODO: Visual: Screen, Color, Capture.

// pub struct Snapshot {}
// pub struct Capture {}

use std::convert::Infallible;

use spire_core::backend::Backend;
use spire_core::context::{Context, Request, Response};
use spire_core::dataset::util::BoxCloneDataset;
use spire_core::BoxError;

use crate::extract::{FromContext, FromContextParts};

#[async_trait::async_trait]
impl<B, S> FromContextParts<B, S> for Context<B> {
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<B>, _state: &S) -> Result<Self, Self::Rejection> {
        // Ok(cx.clone())

        todo!()
    }
}

#[async_trait::async_trait]
impl<B, S> FromContextParts<B, S> for Request {
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<B>, _state: &S) -> Result<Self, Self::Rejection> {
        todo!()
    }
}

#[async_trait::async_trait]
impl<B, S> FromContext<B, S> for Response
where
    B: Backend,
{
    type Rejection = Infallible;

    async fn from_context(cx: Context<B>, _state: &S) -> Result<Self, Self::Rejection> {
        todo!()
    }
}

pub struct Dataset<T>(pub BoxCloneDataset<T, BoxError>);

#[async_trait::async_trait]
impl<B, S, T> FromContextParts<B, S> for Dataset<T> {
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<B>, _state: &S) -> Result<Self, Self::Rejection> {
        todo!()
    }
}

// TODO: Deref into BoxDataset<T>;
