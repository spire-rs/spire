use std::convert::Infallible;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use spire_core::backend::WebDriverPool;
use spire_core::context::Context;

use crate::extract::FromContextParts;

pub struct BrowserHandler<T> {
    marker: PhantomData<T>,
}

impl<T> Deref for BrowserHandler<T> {
    type Target = ();

    fn deref(&self) -> &Self::Target {
        todo!()
    }
}

impl<T> DerefMut for BrowserHandler<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        todo!()
    }
}

pub struct Browser<T>(pub BrowserHandler<T>);

#[async_trait::async_trait]
impl<S, T> FromContextParts<WebDriverPool, S> for Browser<T> {
    type Rejection = Infallible;

    async fn from_context_parts(
        cx: &Context<WebDriverPool>,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        todo!()
    }
}
