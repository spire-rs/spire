use std::convert::Infallible;
use std::ops::{Deref, DerefMut};

use spire_core::context::Context;

use crate::extract::FromContextParts;

/// TODO.
pub trait FromRef<T> {
    /// TODO.
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

/// TODO.
#[derive(Debug, Default, Clone, Copy)]
pub struct State<T>(pub T);

#[async_trait::async_trait]
impl<B, S, T> FromContextParts<B, S> for State<T>
where
    S: Send + Sync + 'static,
    T: FromRef<S>,
{
    type Rejection = Infallible;

    async fn from_context_parts(_cx: &Context<B>, state: &S) -> Result<Self, Self::Rejection> {
        let state = T::from_ref(state);
        Ok(Self(state))
    }
}

impl<T> Deref for State<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for State<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
