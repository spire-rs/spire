use std::convert::Infallible;
use std::ops::{Deref, DerefMut};

use spire_core::backend::Client as CoreClient;
use spire_core::context::{Context, RequestQueue, Tag, Task};
use spire_core::dataset::Dataset as CoreDataset;
use spire_core::dataset::util::BoxCloneDataset;
use spire_core::Error;

use crate::extract::{FromContext, FromContextRef};

/// TODO.
#[derive(Clone)]
pub struct Client<C>(pub C);

#[async_trait::async_trait]
impl<C, S> FromContextRef<C, S> for Client<C>
where
    C: CoreClient + Sync + Clone,
{
    type Rejection = Infallible;

    #[inline]
    async fn from_context_parts(cx: &Context<C>, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Client(cx.client()))
    }
}

impl<C> Deref for Client<C> {
    type Target = C;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<C> DerefMut for Client<C> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

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

/// TODO.
///
/// TODO: Rename to dataset? Move to core and impl FromCtxRef here?
pub struct Dataset2<T>(pub BoxCloneDataset<T, Error>);

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
impl<B, S, T> FromContextRef<B, S> for Dataset2<T>
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
impl<T> CoreDataset<T> for Dataset2<T>
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
