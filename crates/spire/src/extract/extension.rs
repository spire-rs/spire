use std::future::{Future, ready};

use derive_more::{Deref, DerefMut};

use crate::extract::FromContext;
use crate::{Context, Error};

/// A service attached to the request by the crawler or middleware.
///
/// Register services with [`Crawler::with_extension`](crate::Crawler::with_extension).
/// Unlike [`State`](super::State), extensions are looked up by type at runtime.
/// Missing values fail extraction; use `Option<Extension<T>>` for optional services.
///
/// ```
/// use spire::extract::{Extension, State};
/// use spire::{Crawler, Router};
/// use spire::backend::Noop;
///
/// async fn page(State(prefix): State<String>, Extension(limit): Extension<usize>) {
///     println!("{prefix}: {limit}");
/// }
///
/// let router = Router::with_state(String::from("items")).fallback(page);
/// let crawler = Crawler::new(Noop, router).with_extension(10_usize);
/// # let _ = crawler;
/// ```
#[derive(Debug, Default, Clone, Copy, Deref, DerefMut)]
pub struct Extension<T>(pub T);

impl<C, S, T> FromContext<C, S> for Extension<T>
where
    C: Sync,
    S: Sync,
    T: Clone + Send + Sync + 'static,
{
    type Rejection = Error;

    fn from_context(
        cx: &Context<C>,
        _state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        ready(
            cx.request()
                .extensions()
                .get::<T>()
                .cloned()
                .map(Self)
                .ok_or_else(|| {
                    Error::worker(format!(
                        "missing request extension `{}`",
                        std::any::type_name::<T>()
                    ))
                }),
        )
    }
}
