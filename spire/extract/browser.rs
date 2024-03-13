use std::convert::Infallible;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use spire_core::backend::BrowserPool;
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
impl<S, T> FromContextParts<BrowserPool, S> for Browser<T> {
    type Rejection = Infallible;

    async fn from_context_parts(
        cx: &Context<BrowserPool>,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        todo!()
    }
}
