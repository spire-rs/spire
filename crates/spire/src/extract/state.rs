use std::convert::Infallible;

use derive_more::{Deref, DerefMut};

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
#[derive(Debug, Default, Clone, Copy, Deref, DerefMut)]
pub struct State<T>(pub T);

#[spire_core::async_trait]
impl<C, S, T> FromContextRef<C, S> for State<T>
where
    S: Send + Sync + 'static,
    T: FromRef<S>,
{
    type Rejection = Infallible;

    async fn from_context_ref(_cx: &Context<C>, state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(T::from_ref(state)))
    }
}
