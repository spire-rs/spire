//! Extractors: typed handler arguments taken from the [`Context`].
//!
//! A handler is an async function whose arguments are extractors, which
//! implement [`FromContext`]. They borrow the context, so any of them may
//! read the response: the request is fetched once, by the first that needs
//! it, and body extractors share the cached response snapshot.
//!
//! - Request and crawl: [`Uri`](crate::http::http::Uri), [`Query`],
//!   [`Tag`], an enum deriving [`Tags`], [`Depth`], [`Attempt`], [`Queue`]
//!   and [`Data`].
//! - Response: [`Head`] for the status and headers, and for the body
//!   [`Body`], [`Text`] and [`Json`].
//! - HTML, with the `html` feature, in [`html`]: the parsed [`Html`],
//!   [`Element`]s extracted with [`Select`], [`Meta`] and [`JsonLd`].
//! - Sitemaps, with the `sitemap` feature, in [`sitemap`]; and a browser
//!   [`Tab`] with the `thirtyfour` feature.
//! - Everything else: [`State`].
//!
//! Wrapping an extractor in `Option` or `Result` makes it infallible.
//!
//! [`Html`]: html::Html
//! [`Element`]: html::Element
//! [`Select`]: html::Select
//! [`Meta`]: html::Meta
//! [`JsonLd`]: html::JsonLd
//! [`Tag`]: crate::crawler::Tag
//! [`Tags`]: crate::crawler::Tags
//! [`Depth`]: crate::crawler::Depth
//! [`Attempt`]: crate::crawler::Attempt
//! [`Queue`]: crate::frontier::queue::Queue
//! [`Data`]: crate::dataset::Data

#[cfg(feature = "agent")]
mod agent;
mod body;
#[cfg(feature = "thirtyfour")]
mod browser;
pub(crate) mod context;
mod head;
#[cfg(feature = "html")]
#[cfg_attr(docsrs, doc(cfg(feature = "html")))]
pub mod html;
mod query;
#[cfg(feature = "sitemap")]
#[cfg_attr(docsrs, doc(cfg(feature = "sitemap")))]
pub mod sitemap;
mod state;

use std::convert::Infallible;
use std::future::Future;

pub use body::{Body, Json, Text};
#[cfg(feature = "thirtyfour")]
#[cfg_attr(docsrs, doc(cfg(feature = "thirtyfour")))]
pub use browser::Tab;
pub use head::Head;
pub use query::Query;
#[cfg(feature = "macros")]
#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
pub use spire_macros::FromRef;
pub use state::{FromRef, State};

use crate::Context;
use crate::worker::IntoOutcome;

/// An extractor: a handler argument taken from the [`Context`] and the
/// router's state.
pub trait FromContext<C, S>: Sized {
    /// What the handler returns when extraction fails.
    type Rejection: IntoOutcome;

    /// Extracts `Self` from the context and handler state.
    fn from_context(
        cx: &Context<C>,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send;
}

impl<C, S, T> FromContext<C, S> for Option<T>
where
    C: Sync,
    S: Sync,
    T: FromContext<C, S>,
{
    type Rejection = Infallible;

    async fn from_context(cx: &Context<C>, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_context(cx, state).await.ok())
    }
}

impl<C, S, T> FromContext<C, S> for Result<T, T::Rejection>
where
    C: Sync,
    S: Sync,
    T: FromContext<C, S, Rejection: Send>,
{
    type Rejection = Infallible;

    async fn from_context(cx: &Context<C>, state: &S) -> Result<Self, Self::Rejection> {
        Ok(T::from_context(cx, state).await)
    }
}
