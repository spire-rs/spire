use std::convert::Infallible;

pub use content::{Body, Html, Json, Text};
pub use queue::{DataQueue, TaskQueue};
pub use state::{FromRef, State};

use crate::handler::{HandlerContext, IntoControlFlow};

#[cfg(feature = "client")]
pub mod client;
mod content;
#[cfg(feature = "driver")]
pub mod driver;
mod queue;
mod state;
// TODO: mod language;

mod private {
    #[derive(Debug, Clone, Copy)]
    pub enum ViaParts {}

    #[derive(Debug, Clone, Copy)]
    pub enum ViaRequest {}
}

#[async_trait::async_trait]
pub trait FromContextParts<S>: Sized {
    type Rejection: IntoControlFlow;

    async fn from_context_parts(cx: &HandlerContext, state: &S) -> Result<Self, Self::Rejection>;
}

#[async_trait::async_trait]
pub trait FromContext<S, V = private::ViaRequest>: Sized {
    type Rejection: IntoControlFlow;

    async fn from_context(cx: HandlerContext, state: &S) -> Result<Self, Self::Rejection>;
}

#[async_trait::async_trait]
impl<S, T> FromContext<S, private::ViaParts> for T
where
    S: Send + Sync,
    T: FromContextParts<S>,
{
    type Rejection = <Self as FromContextParts<S>>::Rejection;

    async fn from_context(cx: HandlerContext, state: &S) -> Result<Self, Self::Rejection> {
        Self::from_context_parts(&cx, state).await
    }
}

#[async_trait::async_trait]
impl<S, T> FromContextParts<S> for Option<T>
where
    S: Send + Sync,
    T: FromContextParts<S>,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &HandlerContext, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_context_parts(cx, state).await.ok())
    }
}

#[async_trait::async_trait]
impl<S, T> FromContext<S> for Option<T>
where
    S: Send + Sync,
    T: FromContext<S>,
{
    type Rejection = Infallible;

    async fn from_context(cx: HandlerContext, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_context(cx, state).await.ok())
    }
}

#[async_trait::async_trait]
impl<S, T> FromContextParts<S> for Result<T, T::Rejection>
where
    S: Send + Sync,
    T: FromContextParts<S>,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &HandlerContext, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_context_parts(cx, state).await)
    }
}

#[async_trait::async_trait]
impl<S, T> FromContext<S> for Result<T, T::Rejection>
where
    S: Send + Sync,
    T: FromContext<S>,
{
    type Rejection = Infallible;

    async fn from_context(cx: HandlerContext, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_context(cx, state).await)
    }
}
