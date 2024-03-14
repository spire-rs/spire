// TODO: Visual: Screen, Color, Capture.

// pub struct Snapshot {}
// pub struct Capture {}

use std::convert::Infallible;

use spire_core::backend::HttpClient;
use spire_core::context::{Context, RequestQueue, Tag, Task};
use spire_core::dataset::util::BoxCloneDataset;
use spire_core::dataset::Dataset as CoreDataset;
use spire_core::Error;

use crate::extract::{FromContext, FromContextParts};

#[async_trait::async_trait]
impl<B, S> FromContext<B, S> for Context<B>
where
    B: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_context(cx: Context<B>, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(cx)
    }
}

#[async_trait::async_trait]
impl<S> FromContextParts<HttpClient, S> for HttpClient {
    type Rejection = Infallible;

    async fn from_context_parts(
        cx: &Context<HttpClient>,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        todo!()
    }
}

#[async_trait::async_trait]
impl<B, S> FromContextParts<B, S> for RequestQueue
where
    B: Sync,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<B>, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(cx.queue())
    }
}

#[async_trait::async_trait]
impl<B, S> FromContextParts<B, S> for Tag
where
    B: Sync,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<B>, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(cx.peek().tag().clone())
    }
}

pub struct Dataset<T>(pub BoxCloneDataset<T, Error>);

#[async_trait::async_trait]
impl<B, S, T> FromContextParts<B, S> for Dataset<T>
where
    B: Sync,
    T: Send + Sync + 'static,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<B>, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(cx.dataset::<T>()))
    }
}

#[async_trait::async_trait]
impl<T> CoreDataset<T> for Dataset<T>
where
    T: Send + Sync + 'static,
{
    type Error = Error;

    async fn add(&self, data: T) -> Result<(), Self::Error> {
        self.0.add(data).await
    }

    async fn get(&self) -> Result<Option<T>, Self::Error> {
        self.0.get().await
    }

    fn len(&self) -> usize {
        self.0.len()
    }
}
