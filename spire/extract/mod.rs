//! Types and traits for extracting data from [`Context`].
//!
//! ### Intro
//!
//! A [`Handler`] function is an async function that takes any number of extractors as arguments.
//! An extractor is a type that implements [`FromContextRef`] or [`FromContext`].
//!
//! [`Handler`]: crate::handler::Handler
//!
//! ### Extractors
//!
//! - [`Context`] to access [`Request`], and [`Response`] for granular control over data fetching.
//! - [`Body`], [`Text`], and [`Json`] for response body extraction.
//!
//! - [`State`] and [`FromRef`] trait for state extraction.
//!
//! - [`Html`] (for [`HttpClient`]) or [`View`] (for [`BrowserPool`]) for direct markup access,
//! or [`Elements`] and [`Select`] trait for declarative markup extraction.
//! - [`RequestQueue`], and [`Datastore`] for enqueuing new requests and saving response data.
//! - [`Backend`]-specific [`HttpClient`] and [`BrowserClient`] (for [`BrowserPool`]).
//!
//! [`Backend`]: spire_core::backend::Backend
//! [`HttpClient`]: spire_core::backend::HttpClient
//! [`BrowserPool`]: spire_core::backend::BrowserPool
//! [`BrowserClient`]: spire_core::backend::BrowserClient
//!
//! [`Request`]: spire_core::context::Request
//! [`Response`]: spire_core::context::Response
//! [`RequestQueue`]: spire_core::context::RequestQueue

use std::convert::Infallible;

use spire_core::context::{Context, IntoSignal};

pub use crate::extract::content::{Body, Json, Text};
pub use crate::extract::context::*;
pub use crate::extract::state::{FromRef, State};

mod content;
mod context;
mod state;

mod sealed {
    #[derive(Debug, Clone, Copy)]
    pub enum ViaParts {}

    #[derive(Debug, Clone, Copy)]
    pub enum ViaRequest {}
}

/// Core trait for a non-consuming extractor.
#[async_trait::async_trait]
pub trait FromContextRef<B, S>: Sized {
    /// Extraction failure type.
    ///
    /// Should be convertable into the [`Signal`].
    ///
    /// [`Signal`]: crate::context::Signal
    type Rejection: IntoSignal;

    /// Extracts the value from the reference to the context.
    async fn from_context_parts(cx: &Context<B>, state: &S) -> Result<Self, Self::Rejection>;
}

/// Core trait for a non-consuming extractor.
#[async_trait::async_trait]
pub trait FromContext<B, S, V = sealed::ViaRequest>: Sized {
    /// Extraction failure type.
    ///
    /// Should be convertable into the [`Signal`].
    ///
    /// [`Signal`]: crate::context::Signal
    type Rejection: IntoSignal;

    /// Extracts the value from the context.
    async fn from_context(cx: Context<B>, state: &S) -> Result<Self, Self::Rejection>;
}

#[async_trait::async_trait]
impl<B, S, T> FromContext<B, S, sealed::ViaParts> for T
where
    B: Sync + Send + 'static,
    S: Sync + Send + 'static,
    T: FromContextRef<B, S>,
{
    type Rejection = <Self as FromContextRef<B, S>>::Rejection;

    async fn from_context(cx: Context<B>, state: &S) -> Result<Self, Self::Rejection> {
        Self::from_context_parts(&cx, state).await
    }
}

#[async_trait::async_trait]
impl<B, S, T> FromContextRef<B, S> for Option<T>
where
    B: Sync + Send + 'static,
    S: Sync + Send + 'static,
    T: FromContextRef<B, S>,
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
impl<B, S, T> FromContextRef<B, S> for Result<T, T::Rejection>
where
    B: Sync + Send + 'static,
    S: Sync + Send + 'static,
    T: FromContextRef<B, S>,
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
