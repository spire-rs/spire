use std::any::type_name;
use std::convert::Infallible;
use std::future::{Future, ready};

use crate::dataset::Data;
use crate::extract::FromContext;
use crate::frontier::queue::Queue;
use crate::request::http::Uri;
use crate::request::{Attempt, Depth, RequestExt, Tag, Tags};
use crate::{Context, Error, Result};

impl<C, S> FromContext<C, S> for Uri
where
    C: Sync,
    S: Sync,
{
    type Rejection = Infallible;

    fn from_context(
        cx: &Context<C>,
        _: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        ready(Ok(cx.request().uri().clone()))
    }
}

impl<C, S> FromContext<C, S> for Queue
where
    C: Sync,
    S: Sync,
{
    type Rejection = Infallible;

    fn from_context(
        cx: &Context<C>,
        _: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        ready(Ok(cx.queue().clone()))
    }
}

/// Rejects untagged requests; extract `Option<Tag>` in a fallback handler.
impl<C, S> FromContext<C, S> for Tag
where
    C: Sync,
    S: Sync,
{
    type Rejection = Error;

    fn from_context(cx: &Context<C>, _: &S) -> impl Future<Output = Result<Self>> + Send {
        ready({
            let tag = cx.request().tag().cloned();
            tag.ok_or_else(|| Error::worker("request has no tag"))
        })
    }
}

/// Reads the request's tag as a member of `T`; called by the extractor
/// `#[derive(Tags)]` generates.
///
/// # Errors
///
/// Returns an error if the request is untagged or its tag names no member.
pub fn tag_of<T: Tags, C>(cx: &Context<C>) -> Result<T> {
    let Some(tag) = cx.request().tag() else {
        return Err(Error::worker("request has no tag"));
    };
    let name = type_name::<T>().rsplit("::").next().unwrap_or_default();
    T::from_tag(tag).ok_or_else(|| Error::worker(format!("tag `{tag}` is not a `{name}`")))
}

impl<C, S> FromContext<C, S> for Depth
where
    C: Sync,
    S: Sync,
{
    type Rejection = Infallible;

    fn from_context(
        cx: &Context<C>,
        _: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        ready(Ok(cx.request().depth()))
    }
}

impl<C, S> FromContext<C, S> for Attempt
where
    C: Sync,
    S: Sync,
{
    type Rejection = Infallible;

    fn from_context(
        cx: &Context<C>,
        _: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        ready(Ok(cx.request().attempt()))
    }
}

/// Rejects with a [`Dataset`](crate::ErrorKind::Dataset) error if no dataset
/// is registered for `T` on the crawler.
impl<C, S, T> FromContext<C, S> for Data<T>
where
    C: Sync,
    S: Sync,
    T: Send + 'static,
{
    type Rejection = Error;

    fn from_context(cx: &Context<C>, _: &S) -> impl Future<Output = Result<Self>> + Send {
        ready(cx.dataset::<T>())
    }
}
