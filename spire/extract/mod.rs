use std::convert::Infallible;

use spire_core::context::Context;
use spire_core::process::IntoSignal;

pub use crate::extract::content::{Body, Json, Text};
pub use crate::extract::context::Dataset;
pub use crate::extract::markup::{Html, Nest, Select, Selector};
pub use crate::extract::state::{FromRef, State};

mod content;
mod context;
mod markup;
mod state;

mod sealed {
    #[derive(Debug, Clone, Copy)]
    pub enum ViaParts {}

    #[derive(Debug, Clone, Copy)]
    pub enum ViaRequest {}
}

#[async_trait::async_trait]
pub trait FromContextParts<B, S>: Sized {
    type Rejection: IntoSignal;

    async fn from_context_parts(cx: &Context<B>, state: &S) -> Result<Self, Self::Rejection>;
}

#[async_trait::async_trait]
pub trait FromContext<B, S, V = sealed::ViaRequest>: Sized {
    type Rejection: IntoSignal;

    async fn from_context(cx: Context<B>, state: &S) -> Result<Self, Self::Rejection>;
}

#[async_trait::async_trait]
impl<B, S, T> FromContext<B, S, sealed::ViaParts> for T
where
    B: Sync + Send + 'static,
    S: Sync + Send + 'static,
    T: FromContextParts<B, S>,
{
    type Rejection = <Self as FromContextParts<B, S>>::Rejection;

    async fn from_context(cx: Context<B>, state: &S) -> Result<Self, Self::Rejection> {
        Self::from_context_parts(&cx, state).await
    }
}

#[async_trait::async_trait]
impl<B, S, T> FromContextParts<B, S> for Option<T>
where
    B: Sync + Send + 'static,
    S: Sync + Send + 'static,
    T: FromContextParts<B, S>,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<B>, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_context_parts(cx, state).await.ok())
    }
}

#[async_trait::async_trait]
impl<B, S, T> FromContext<B, S> for Option<T>
where
    B: Sync + Send + 'static,
    S: Sync + Send + 'static,
    T: FromContext<B, S>,
{
    type Rejection = Infallible;

    async fn from_context(cx: Context<B>, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_context(cx, state).await.ok())
    }
}

#[async_trait::async_trait]
impl<B, S, T> FromContextParts<B, S> for Result<T, T::Rejection>
where
    B: Sync + Send + 'static,
    S: Sync + Send + 'static,
    T: FromContextParts<B, S>,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<B>, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_context_parts(cx, state).await)
    }
}

#[async_trait::async_trait]
impl<B, S, T> FromContext<B, S> for Result<T, T::Rejection>
where
    B: Sync + Send + 'static,
    S: Sync + Send + 'static,
    T: FromContext<B, S>,
{
    type Rejection = Infallible;

    async fn from_context(cx: Context<B>, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_context(cx, state).await)
    }
}
