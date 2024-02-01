use std::convert::Infallible;
use std::ops::{Deref, DerefMut};

use spire_core::collect::HandlerContext;

use crate::extract::{FromContext, FromContextParts};

pub trait FromRef<T> {
    fn from_ref(input: &T) -> Self;
}

impl<T> FromRef<T> for T
where
    T: Clone,
{
    fn from_ref(input: &T) -> Self {
        input.clone()
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct State<T>(pub T);

#[async_trait::async_trait]
impl<S, T> FromContextParts<S> for State<T>
where
    T: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_context_parts(_cx: &HandlerContext, state: &S) -> Result<Self, Self::Rejection> {
        let state = T::from_ref(state);
        Ok(Self(state))
    }
}

impl<S> Deref for State<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<S> DerefMut for State<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
