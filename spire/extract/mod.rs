//! Types and traits for extracting data from [`Context`].
//!
//! ### Intro
//!
//! A [`Handler`] function is an async function that takes any number of extractors as arguments.
//! An extractor is a type that implements [`FromContext`] or [`FromContextParts`].
//!
//! ### Extractors
//!
//! - [`Context`] to access [`Request`], and [`Response`] for granular control over data fetching.
//! - [`Body`], [`Text`], and [`Json`]
//! - [`Html`], [`Elements`] and [`Select`] trait for declarative markup search and extraction.
//! - [`Queue`], and [`Dataset`] for creating new requests and saving scraped data.
//! - [`State`] and [`FromRef`] trait for state extraction.
//!
//! - [`Backend`]-specific [`HttpClient`] and [`WebDriver`].
//! TODO: Browser, Client.
//!
//! [`Backend`]: spire_core::backend::Backend
//! [`Request`]: spire_core::context::Request
//! [`Response`]: spire_core::context::Response
//! [`Tag`]: spire_core::context::Tag
//! [`Queue`]: spire_core::context::RequestQueue
//! [`Handler`]: crate::handler::Handler

use std::convert::Infallible;

pub use content::{Body, Html, Json, Text};
pub use context::Dataset;
#[cfg(feature = "macros")]
pub use macros::{Elements, Select};
use spire_core::context::{Context, IntoSignal};
pub use state::{FromRef, State};

mod browser;
mod content;
mod context;
#[cfg(feature = "macros")]
mod macros;
mod state;

mod sealed {
    #[derive(Debug, Clone, Copy)]
    pub enum ViaParts {}

    #[derive(Debug, Clone, Copy)]
    pub enum ViaRequest {}
}

/// TODO.
#[async_trait::async_trait]
pub trait FromContextParts<B, S>: Sized {
    /// TODO.
    type Rejection: IntoSignal;

    /// TODO.
    async fn from_context_parts(cx: &Context<B>, state: &S) -> Result<Self, Self::Rejection>;
}

/// TODO.
#[async_trait::async_trait]
pub trait FromContext<B, S, V = sealed::ViaRequest>: Sized {
    /// TODO.
    type Rejection: IntoSignal;

    /// TODO.
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
