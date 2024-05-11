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
//! - [`Context`] to access [`Request`] or [`Response`] for granular control over data fetching.
//! - [`Body`], [`Text`], and [`Json`] for response body extraction.
//! - [`Html`] (for [`HttpClient`]) or [`View`] (for [`BrowserClient`]) for direct markup access.
//! - [`Elements`] and [`Select`] trait for declarative markup extraction.
//!
//! - [`RequestQueue`], and [`Datastore`] for enqueuing new requests and saving response data.
//! - [`Client`] to access [`Backend`]-specific [`HttpClient`] or [`BrowserClient`].
//! - [`State`] and [`FromRef`] trait for state extraction.
//!
//! [`Backend`]: crate::backend::Backend
//! [`HttpClient`]: crate::backend::HttpClient
//! [`BrowserClient`]: crate::backend::BrowserClient
//!
//! [`Request`]: crate::context::Request
//! [`Response`]: crate::context::Response
//! [`RequestQueue`]: crate::context::RequestQueue
//! [`Datastore`]: crate::dataset::Data
//!
//! [`Html`]: client::Html
//! [`View`]: driver::View

use std::convert::Infallible;

#[cfg(feature = "macros")]
#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
pub use spire_macros::extract::{Elements, Select};

use crate::context::{Context, IntoSignal};
pub use crate::extract::content::{Body, Json, Text};
pub use crate::extract::context::Client;
pub use crate::extract::state::{FromRef, State};

mod content;
mod context;
mod state;

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub mod client;
#[cfg(feature = "driver")]
#[cfg_attr(docsrs, doc(cfg(feature = "driver")))]
pub mod driver;

mod sealed {
    #[derive(Debug, Clone, Copy)]
    pub enum ViaParts {}

    #[derive(Debug, Clone, Copy)]
    pub enum ViaRequest {}
}

/// Core trait for a non-consuming extractor.
#[async_trait::async_trait]
pub trait FromContextRef<C, S>: Sized {
    /// Extraction failure type.
    ///
    /// Should be convertable into the [`Signal`].
    ///
    /// [`Signal`]: crate::context::Signal
    type Rejection: IntoSignal;

    /// Extracts the value from the reference to the context.
    async fn from_context_parts(cx: &Context<C>, state: &S) -> Result<Self, Self::Rejection>;
}

/// Core trait for a consuming extractor.
#[async_trait::async_trait]
pub trait FromContext<C, S, V = sealed::ViaRequest>: Sized {
    /// Extraction failure type.
    ///
    /// Should be convertable into the [`Signal`].
    ///
    /// [`Signal`]: crate::context::Signal
    type Rejection: IntoSignal;

    /// Extracts the value from the context.
    async fn from_context(cx: Context<C>, state: &S) -> Result<Self, Self::Rejection>;
}

#[async_trait::async_trait]
impl<C, S, T> FromContext<C, S, sealed::ViaParts> for T
where
    C: Send + Sync + 'static,
    S: Send + Sync + 'static,
    T: FromContextRef<C, S>,
{
    type Rejection = <Self as FromContextRef<C, S>>::Rejection;

    async fn from_context(cx: Context<C>, state: &S) -> Result<Self, Self::Rejection> {
        Self::from_context_parts(&cx, state).await
    }
}

#[async_trait::async_trait]
impl<C, S, T> FromContextRef<C, S> for Option<T>
where
    C: Send + Sync + 'static,
    S: Send + Sync + 'static,
    T: FromContextRef<C, S>,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<C>, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_context_parts(cx, state).await.ok())
    }
}

#[async_trait::async_trait]
impl<C, S, T> FromContext<C, S> for Option<T>
where
    C: Send + Sync + 'static,
    S: Send + Sync + 'static,
    T: FromContext<C, S>,
{
    type Rejection = Infallible;

    async fn from_context(cx: Context<C>, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_context(cx, state).await.ok())
    }
}

#[async_trait::async_trait]
impl<C, S, T> FromContextRef<C, S> for Result<T, T::Rejection>
where
    C: Send + Sync + 'static,
    S: Send + Sync + 'static,
    T: FromContextRef<C, S>,
{
    type Rejection = Infallible;

    async fn from_context_parts(cx: &Context<C>, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_context_parts(cx, state).await)
    }
}

#[async_trait::async_trait]
impl<C, S, T> FromContext<C, S> for Result<T, T::Rejection>
where
    C: Send + Sync + 'static,
    S: Send + Sync + 'static,
    T: FromContext<C, S>,
{
    type Rejection = Infallible;

    async fn from_context(cx: Context<C>, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_context(cx, state).await)
    }
}
