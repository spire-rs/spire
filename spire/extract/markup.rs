use std::convert::Infallible;
use std::ops::{Deref, DerefMut};

use spire_core::backend::Backend;
use spire_core::context::Context;

use crate::extract::FromContextParts;

/// TODO.
#[derive(Debug, Clone)]
pub struct Html(pub ());

#[async_trait::async_trait]
impl<B, S> FromContextParts<B, S> for Html
where
    B: Backend,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<B>, _state: &S) -> Result<Self, Self::Rejection> {
        todo!()
    }
}

/// TODO.
#[derive(Debug, Clone)]
pub struct Nest(pub ());

#[async_trait::async_trait]
impl<B, S> FromContextParts<B, S> for Nest
where
    B: Backend,
    S: Sync,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<B>, state: &S) -> Result<Self, Self::Rejection> {
        let html = Html::from_context_parts(cx, state).await;
        todo!()
    }
}

/// TODO.
pub trait Select {}

/// TODO.
pub struct Selector<T>(pub T);

#[async_trait::async_trait]
impl<B, S, T> FromContextParts<B, S> for Selector<T>
where
    B: Backend,
    S: Sync,
    T: Select,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<B>, state: &S) -> Result<Self, Self::Rejection> {
        let html = Html::from_context_parts(cx, state).await;
        todo!()
    }
}

impl<T> Deref for Selector<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Selector<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
