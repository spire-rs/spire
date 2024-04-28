use std::convert::Infallible;

use spire_core::context::{Context, RequestQueue, Tag, Task};
use spire_core::dataset::{Dataset, util::BoxCloneDataset};
use spire_core::Error;

use crate::extract::{FromContext, FromContextRef};
#[cfg(feature = "client")]
pub use crate::extract::context::client::Html;
#[cfg(feature = "driver")]
pub use crate::extract::context::driver::{Browser, BrowserHandle, View};

#[cfg(feature = "client")]
mod client;
#[cfg(feature = "driver")]
mod driver;

#[async_trait::async_trait]
impl<B, S> FromContext<B, S> for Context<B>
where
    B: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_context(cx: Context<B>, _: &S) -> Result<Self, Self::Rejection> {
        Ok(cx)
    }
}

#[async_trait::async_trait]
impl<B, S> FromContextRef<B, S> for RequestQueue
where
    B: Sync,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<B>, _: &S) -> Result<Self, Self::Rejection> {
        Ok(cx.queue())
    }
}

#[async_trait::async_trait]
impl<B, S> FromContextRef<B, S> for Tag
where
    B: Sync,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<B>, _: &S) -> Result<Self, Self::Rejection> {
        Ok(cx.get_ref().tag().clone())
    }
}

/// TODO. Dataset?
pub struct Data<T>(pub BoxCloneDataset<T, Error>);

#[async_trait::async_trait]
impl<B, S, T> FromContextRef<B, S> for BoxCloneDataset<T, Error>
where
    B: Sync,
    T: Sync + Send + 'static,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<B>, _: &S) -> Result<Self, Self::Rejection> {
        Ok(cx.dataset::<T>())
    }
}

#[async_trait::async_trait]
impl<B, S, T> FromContextRef<B, S> for Data<T>
where
    B: Sync,
    T: Sync + Send + 'static,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<B>, _: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(cx.dataset::<T>()))
    }
}

#[async_trait::async_trait]
impl<T> Dataset<T> for Data<T>
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
