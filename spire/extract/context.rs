use std::convert::Infallible;
use std::ops::{Deref, DerefMut};

use spire_core::backend::Client as CoreClient;
use spire_core::context::{Context, RequestQueue, Tag, Task};
use spire_core::dataset::{Data, Dataset as CoreDataset};

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

#[async_trait::async_trait]
impl<B, S, T> FromContextRef<B, S> for Data<T>
where
    B: Sync,
    T: Sync + Send + 'static,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<B>, _: &S) -> Result<Self, Self::Rejection> {
        Ok(cx.dataset::<T>())
    }
}
