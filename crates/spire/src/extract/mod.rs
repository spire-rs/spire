//! Extractors: typed handler arguments taken from the [`Context`].
//!
//! A handler is an async function whose arguments are extractors. Every
//! argument but the last implements [`FromContextRef`] and reads the context;
//! the last may implement [`FromContext`] and consume it, typically to fetch
//! the response.
//!
//! - Request and crawl: [`http::Uri`](crate::http::Uri), [`Tag`],
//!   [`Depth`], [`Attempt`], [`Queue`] and [`Data`].
//! - Response body: [`Bytes`], [`Text`], [`Json`], and with the `html`
//!   feature [`Html`], [`Element`] and [`Elements`].
//! - Everything else: [`State`], [`Client`] and the [`Context`] itself.
//!
//! Wrapping an extractor in `Option` or `Result` makes it infallible.
//!
//! [`Tag`]: crate::request::Tag
//! [`Depth`]: crate::request::Depth
//! [`Attempt`]: crate::request::Attempt
//! [`Queue`]: crate::frontier::Queue
//! [`Data`]: crate::dataset::Data

mod content;
mod context;
#[cfg(feature = "thirtyfour")]
#[cfg_attr(docsrs, doc(cfg(feature = "thirtyfour")))]
pub mod driver;
#[cfg(feature = "html")]
pub(crate) mod html;
mod state;

use std::convert::Infallible;
use std::future::Future;

pub use content::{Bytes, Json, Text};
pub use context::Client;
#[cfg(feature = "html")]
#[cfg_attr(docsrs, doc(cfg(feature = "html")))]
pub use html::{Element, Elements, Html, Select};
#[cfg(feature = "macros")]
#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
pub use spire_macros::Select;
pub use state::{FromRef, State};

use crate::Context;
use crate::worker::IntoOutcome;

/// Marker types that tell [`FromContext`] implementations apart.
mod private {
    /// The extractor reads the context through [`FromContextRef`](super::FromContextRef).
    #[derive(Debug, Clone, Copy)]
    pub enum ViaRef {}

    /// The extractor consumes the context.
    #[derive(Debug, Clone, Copy)]
    pub enum ViaContext {}
}

/// An extractor that reads the [`Context`] without consuming it.
pub trait FromContextRef<C, S>: Sized {
    /// What the handler returns when extraction fails.
    type Rejection: IntoOutcome;

    /// Extracts `Self` from the context and handler state.
    fn from_context_ref(
        cx: &Context<C>,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send;
}

/// An extractor that consumes the [`Context`], e.g. to fetch the response.
///
/// Only the last argument of a handler can consume the context.
pub trait FromContext<C, S, M = private::ViaContext>: Sized {
    /// What the handler returns when extraction fails.
    type Rejection: IntoOutcome;

    /// Extracts `Self` from the context and handler state.
    fn from_context(
        cx: Context<C>,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send;
}

impl<C, S, T> FromContext<C, S, private::ViaRef> for T
where
    C: Send + Sync,
    S: Sync,
    T: FromContextRef<C, S>,
{
    type Rejection = T::Rejection;

    async fn from_context(cx: Context<C>, state: &S) -> Result<Self, Self::Rejection> {
        T::from_context_ref(&cx, state).await
    }
}

impl<C, S, T> FromContextRef<C, S> for Option<T>
where
    C: Sync,
    S: Sync,
    T: FromContextRef<C, S>,
{
    type Rejection = Infallible;

    async fn from_context_ref(cx: &Context<C>, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_context_ref(cx, state).await.ok())
    }
}

impl<C, S, T> FromContext<C, S> for Option<T>
where
    C: Send,
    S: Sync,
    T: FromContext<C, S>,
{
    type Rejection = Infallible;

    async fn from_context(cx: Context<C>, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_context(cx, state).await.ok())
    }
}

impl<C, S, T> FromContextRef<C, S> for Result<T, T::Rejection>
where
    C: Sync,
    S: Sync,
    T: FromContextRef<C, S, Rejection: Send>,
{
    type Rejection = Infallible;

    async fn from_context_ref(cx: &Context<C>, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_context_ref(cx, state).await)
    }
}

impl<C, S, T> FromContext<C, S> for Result<T, T::Rejection>
where
    C: Send,
    S: Sync,
    T: FromContext<C, S, Rejection: Send>,
{
    type Rejection = Infallible;

    async fn from_context(cx: Context<C>, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_context(cx, state).await)
    }
}
