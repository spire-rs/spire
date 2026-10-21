use std::convert::Infallible;
use std::future::{Future, ready};

use derive_more::{Deref, DerefMut};

use crate::Context;
use crate::extract::FromContextRef;

/// Converts a reference to the router state into a part of it.
///
/// Implement it to extract a field of the state with [`State`]:
///
/// ```
/// use spire::extract::FromRef;
///
/// #[derive(Clone)]
/// struct AppState {
///     api_key: String,
/// }
///
/// struct ApiKey(String);
///
/// impl FromRef<AppState> for ApiKey {
///     fn from_ref(state: &AppState) -> Self {
///         Self(state.api_key.clone())
///     }
/// }
/// ```
pub trait FromRef<T> {
    /// Converts `input` into `Self`.
    fn from_ref(input: &T) -> Self;
}

impl<T: Clone> FromRef<T> for T {
    fn from_ref(input: &T) -> Self {
        input.clone()
    }
}

/// Extracts the router state, or a part of it through [`FromRef`].
///
/// ```
/// use spire::Router;
/// use spire::extract::State;
///
/// #[derive(Clone)]
/// struct AppState {
///     api_key: String,
/// }
///
/// async fn handler(State(state): State<AppState>) {
///     println!("{}", state.api_key);
/// }
///
/// let state = AppState { api_key: "secret".to_owned() };
/// let router: Router<(), AppState> = Router::with_state(state).route("page", handler);
/// ```
#[derive(Debug, Default, Clone, Copy, Deref, DerefMut)]
pub struct State<T>(pub T);

impl<C, S, T> FromContextRef<C, S> for State<T>
where
    C: Sync,
    S: Sync,
    T: FromRef<S> + Send,
{
    type Rejection = Infallible;

    fn from_context_ref(
        _cx: &Context<C>,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        ready(Ok(Self(T::from_ref(state))))
    }
}
