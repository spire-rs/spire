use std::convert::Infallible;
use std::future::{Future, ready};

use derive_more::{Deref, DerefMut};

use crate::dataset::Data;
use crate::extract::{FromContext, FromContextRef};
use crate::frontier::Queue;
use crate::http::Uri;
use crate::request::{Attempt, Depth, RequestExt, Tag};
use crate::{Context, Error, Result};

/// Extracts a clone of the backend client handling the request.
#[must_use]
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct Client<C>(pub C);

impl<C, S> FromContextRef<C, S> for Client<C>
where
    C: Clone + Send + Sync,
    S: Sync,
{
    type Rejection = Infallible;

    fn from_context_ref(
        cx: &Context<C>,
        _: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        ready(Ok(Self(cx.client().clone())))
    }
}

impl<C, S> FromContext<C, S> for Context<C>
where
    C: Send,
    S: Sync,
{
    type Rejection = Infallible;

    fn from_context(cx: Self, _: &S) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        ready(Ok(cx))
    }
}

impl<C, S> FromContextRef<C, S> for Uri
where
    C: Sync,
    S: Sync,
{
    type Rejection = Infallible;

    fn from_context_ref(
        cx: &Context<C>,
        _: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        ready(Ok(cx.request().uri().clone()))
    }
}

impl<C, S> FromContextRef<C, S> for Queue
where
    C: Sync,
    S: Sync,
{
    type Rejection = Infallible;

    fn from_context_ref(
        cx: &Context<C>,
        _: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        ready(Ok(cx.queue().clone()))
    }
}

/// Rejects untagged requests; extract `Option<Tag>` in a fallback handler.
impl<C, S> FromContextRef<C, S> for Tag
where
    C: Sync,
    S: Sync,
{
    type Rejection = Error;

    fn from_context_ref(cx: &Context<C>, _: &S) -> impl Future<Output = Result<Self>> + Send {
        ready({
            let tag = cx.request().tag().cloned();
            tag.ok_or_else(|| Error::worker("request has no tag"))
        })
    }
}

impl<C, S> FromContextRef<C, S> for Depth
where
    C: Sync,
    S: Sync,
{
    type Rejection = Infallible;

    fn from_context_ref(
        cx: &Context<C>,
        _: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        ready(Ok(cx.request().depth()))
    }
}

impl<C, S> FromContextRef<C, S> for Attempt
where
    C: Sync,
    S: Sync,
{
    type Rejection = Infallible;

    fn from_context_ref(
        cx: &Context<C>,
        _: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        ready(Ok(cx.request().attempt()))
    }
}

/// Rejects with a [`Dataset`](crate::ErrorKind::Dataset) error if no dataset
/// is registered for `T` on the crawler.
impl<C, S, T> FromContextRef<C, S> for Data<T>
where
    C: Sync,
    S: Sync,
    T: Send + 'static,
{
    type Rejection = Error;

    fn from_context_ref(cx: &Context<C>, _: &S) -> impl Future<Output = Result<Self>> + Send {
        ready(cx.dataset::<T>())
    }
}
