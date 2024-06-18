use std::convert::Infallible;
use std::ops::{Deref, DerefMut};

use crate::context::{Context, RequestQueue, Tag, Task};
use crate::dataset::{future::DataSink, future::DataStream, Data};
use crate::extract::{FromContext, FromContextRef};

/// [`Backend`]-specific client extractor.
///
/// [`Backend`]: crate::backend::Backend
#[must_use]
#[derive(Clone)]
pub struct Client<C>(pub C);

#[async_trait::async_trait]
impl<C, S> FromContextRef<C, S> for Client<C>
where
    C: Clone + Sync,
{
    type Rejection = Infallible;

    #[inline]
    async fn from_context_ref(cx: &Context<C>, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(cx.as_client_ref().clone()))
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
impl<C, S> FromContext<C, S> for Context<C>
where
    C: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_context(cx: Self, _: &S) -> Result<Self, Self::Rejection> {
        Ok(cx)
    }
}

#[async_trait::async_trait]
impl<C, S> FromContextRef<C, S> for RequestQueue
where
    C: Sync,
{
    type Rejection = Infallible;

    async fn from_context_ref(cx: &Context<C>, _: &S) -> Result<Self, Self::Rejection> {
        Ok(cx.queue())
    }
}

#[async_trait::async_trait]
impl<C, S> FromContextRef<C, S> for Tag
where
    C: Sync,
{
    type Rejection = Infallible;

    async fn from_context_ref(cx: &Context<C>, _: &S) -> Result<Self, Self::Rejection> {
        Ok(cx.get_ref().tag().clone())
    }
}

#[async_trait::async_trait]
impl<C, S, T> FromContextRef<C, S> for Data<T>
where
    C: Sync,
    T: Send + Sync + 'static,
{
    type Rejection = Infallible;

    async fn from_context_ref(cx: &Context<C>, _: &S) -> Result<Self, Self::Rejection> {
        Ok(cx.dataset::<T>())
    }
}

#[async_trait::async_trait]
impl<C, S, T> FromContextRef<C, S> for DataStream<T>
where
    C: Sync,
    T: Send + Sync + 'static,
{
    type Rejection = Infallible;

    async fn from_context_ref(cx: &Context<C>, _: &S) -> Result<Self, Self::Rejection> {
        Ok(cx.dataset::<T>().into_stream())
    }
}

#[async_trait::async_trait]
impl<C, S, T> FromContextRef<C, S> for DataSink<T>
where
    C: Sync,
    T: Send + Sync + 'static,
{
    type Rejection = Infallible;

    async fn from_context_ref(cx: &Context<C>, _: &S) -> Result<Self, Self::Rejection> {
        Ok(cx.dataset::<T>().into_sink())
    }
}
