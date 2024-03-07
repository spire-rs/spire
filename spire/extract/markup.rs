use std::convert::Infallible;
use std::ops::{Deref, DerefMut};

use spire_core::backend::Backend;
use spire_core::context::Context;

use crate::extract::FromContextParts;

// TODO: also use for Text?
pub trait Transform<T> {
    type Output;

    fn transform(input: T) -> Self::Output;
}

pub struct Normal;

impl<T> Transform<T> for Normal {
    type Output = T;

    fn transform(input: T) -> Self::Output {
        input
    }
}

// pub struct Reduce;

/// TODO.
#[derive(Debug, Clone)]
pub struct Html<T = Normal>(pub T::Output)
where
    T: Transform<()>;

#[async_trait::async_trait]
impl<B, S, T> FromContextParts<B, S> for Html<T>
where
    B: Backend,
    T: Transform<()>,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<B>, _state: &S) -> Result<Self, Self::Rejection> {
        todo!()
    }
}

/// TODO.
pub trait Select {
    fn list_selected() -> Vec<String>;

    fn from_list(selected: &[String]) -> Self;
}

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
        let html = Html::<Normal>::from_context_parts(cx, state).await;
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
