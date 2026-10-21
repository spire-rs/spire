//! Extractors: typed handler arguments taken from the [`Context`].
//!
//! A handler is an async function whose arguments are extractors, which
//! implement [`FromContext`]. They borrow the context, so any of them may
//! read the response: the request is fetched once, by the first that needs
//! it, and body extractors read the page as it is when they run.
//!
//! - Request and crawl: [`Uri`](crate::request::http::Uri), [`Tag`], an enum
//!   deriving [`Tags`],
//!   [`Depth`], [`Attempt`], [`Queue`] and [`Data`].
//! - Response body: [`Bytes`], [`Text`], [`Json`], and with the `html`
//!   feature [`Html`] and [`Element`].
//! - Everything else: [`State`].
//!
//! Wrapping an extractor in `Option` or `Result` makes it infallible.
//!
//! [`Tag`]: crate::request::Tag
//! [`Tags`]: crate::request::Tags
//! [`Depth`]: crate::request::Depth
//! [`Attempt`]: crate::request::Attempt
//! [`Queue`]: crate::frontier::queue::Queue
//! [`Data`]: crate::dataset::Data

mod content;
pub(crate) mod context;
#[cfg(feature = "thirtyfour")]
mod driver;
#[cfg(feature = "html")]
pub(crate) mod html;
#[cfg(feature = "sitemap")]
mod sitemap;
mod state;

use std::convert::Infallible;
use std::future::Future;

pub use content::{Bytes, Json, Text};
#[cfg(feature = "thirtyfour")]
#[cfg_attr(docsrs, doc(cfg(feature = "thirtyfour")))]
pub use driver::View;
#[cfg(feature = "html")]
#[cfg_attr(docsrs, doc(cfg(feature = "html")))]
pub use html::{Element, Html, Select};
#[cfg(feature = "sitemap")]
#[cfg_attr(docsrs, doc(cfg(feature = "sitemap")))]
pub use sitemap::Sitemap;
/// The sitemap parser behind [`Sitemap`]: its records and their attributes.
#[cfg(feature = "sitemap")]
#[cfg_attr(docsrs, doc(cfg(feature = "sitemap")))]
pub use sitemapo;
#[cfg(feature = "macros")]
#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
pub use spire_macros::Select;
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
