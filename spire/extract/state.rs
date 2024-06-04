use std::convert::Infallible;
use std::ops::{Deref, DerefMut};

use crate::context::Context;
use crate::extract::FromContextRef;

/// Used to do reference-to-value conversion.
pub trait FromRef<T> {
    /// Converts to the `self` from an input reference.
    fn from_ref(input: &T) -> Self;
}

impl<T> FromRef<T> for T
where
    T: Clone,
{
    #[inline]
    fn from_ref(input: &T) -> Self {
        input.clone()
    }
}

/// State extractor.
///
/// ```rust
/// use spire::extract::{FromRef, State};
///
/// #[derive(Debug, Clone)]
/// struct AppState {
///     port: u16,
/// }
///
/// impl FromRef<AppState> for u16 {
///     fn from_ref(input: &AppState) -> Self {
///         input.port
///     }
/// }
///
/// async fn handler(State(port): State<u16>) {}
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct State<T>(pub T);

#[async_trait::async_trait]
impl<C, S, T> FromContextRef<C, S> for State<T>
where
    S: Send + Sync + 'static,
    T: FromRef<S>,
{
    type Rejection = Infallible;

    async fn from_context_parts(_cx: &Context<C>, state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(T::from_ref(state)))
    }
}

impl<T> Deref for State<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for State<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
