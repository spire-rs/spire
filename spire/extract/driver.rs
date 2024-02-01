use core::fmt;
use std::convert::Infallible;
use std::marker::PhantomData;

use spire_core::collect::HandlerContext;

use crate::extract::FromContextParts;

pub trait WebDriver {}

pub struct Chrome {}

impl WebDriver for Chrome {}

pub struct Firefox {}

impl WebDriver for Firefox {}

pub struct Safari {}

impl WebDriver for Safari {}


pub struct BrowserHandler<T> {
    marker: PhantomData<T>,
}

pub struct Browser<T = Chrome>(pub BrowserHandler<T>);

#[async_trait::async_trait]
impl<S, T> FromContextParts<S> for Browser<T> {
    type Rejection = Infallible;

    async fn from_context_parts(cx: &HandlerContext, state: &S) -> Result<Self, Self::Rejection> {
        todo!()
    }
}

impl<T> fmt::Debug for Browser<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}
