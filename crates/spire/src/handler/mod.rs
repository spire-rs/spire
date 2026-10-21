//! Handlers: async functions of extractors that handle a request.

#[cfg(feature = "html")]
mod html;
#[cfg(feature = "sitemap")]
mod sitemap;

use std::future::Future;

#[cfg(feature = "html")]
#[cfg_attr(docsrs, doc(cfg(feature = "html")))]
pub use html::{Collect, FollowLinks};
#[cfg(feature = "sitemap")]
#[cfg_attr(docsrs, doc(cfg(feature = "sitemap")))]
pub use sitemap::FollowSitemap;

use crate::Context;
use crate::extract::FromContext;
use crate::worker::{IntoOutcome, Outcome};

/// Handles a request, usually an async function rather than implemented by
/// hand.
///
/// It is implemented for async functions of up to 16 [extractors] whose
/// return type implements [`IntoOutcome`], such as `()`, `Result<(), Error>`
/// or [`Outcome`]. Handlers borrow the [`Context`], so several can handle one
/// request: a tuple of up to 4 handlers runs them in turn on the same page,
/// fetched once, and stops at the first outcome other than
/// [`Outcome::Continue`], as `?` stops a function at the first error.
///
/// ```
/// use spire::Router;
/// use spire::extract::Text;
/// use spire::request::Tag;
///
/// async fn log(tag: Tag, Text(body): Text) {
///     println!("{tag}: {} bytes", body.len());
/// }
///
/// async fn store(Text(body): Text) -> spire::Result<()> {
///     // ...
///     # let _ = body;
///     Ok(())
/// }
///
/// let router: Router<spire::backend::Noop> = Router::new().route("page", (log, store));
/// ```
///
/// [extractors]: crate::extract
pub trait Handler<C, M, S>: Send + Sync + 'static {
    /// Handles the request in `cx`.
    fn call(&self, cx: &Context<C>, state: &S) -> impl Future<Output = Outcome> + Send;
}

impl<C, S, F, Fut> Handler<C, (), S> for F
where
    C: Sync,
    S: Sync,
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output: IntoOutcome> + Send,
{
    async fn call(&self, _cx: &Context<C>, _state: &S) -> Outcome {
        self().await.into_outcome()
    }
}

macro_rules! impl_handler {
    ($($ty:ident),+) => {
        #[expect(non_snake_case, reason = "extractors are bound to their type parameters' names")]
        impl<C, S, F, Fut, $($ty,)*> Handler<C, ($($ty,)*), S> for F
        where
            C: Sync,
            S: Sync,
            F: Fn($($ty,)*) -> Fut + Send + Sync + 'static,
            Fut: Future<Output: IntoOutcome> + Send,
            $($ty: FromContext<C, S> + Send,)*
        {
            async fn call(&self, cx: &Context<C>, state: &S) -> Outcome {
                $(
                    let $ty = match $ty::from_context(cx, state).await {
                        Ok(value) => value,
                        Err(rejection) => return rejection.into_outcome(),
                    };
                )*
                self($($ty,)*).await.into_outcome()
            }
        }
    };
}

impl_handler!(T1);
impl_handler!(T1, T2);
impl_handler!(T1, T2, T3);
impl_handler!(T1, T2, T3, T4);
impl_handler!(T1, T2, T3, T4, T5);
impl_handler!(T1, T2, T3, T4, T5, T6);
impl_handler!(T1, T2, T3, T4, T5, T6, T7);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13);
impl_handler!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14);
impl_handler!(
    T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15
);
impl_handler!(
    T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16
);

macro_rules! impl_sequence {
    ($(($handler:ident, $marker:ident)),*) => {
        #[expect(non_snake_case, reason = "handlers are bound to their type parameters' names")]
        impl<C, S, $($handler, $marker,)*> Handler<C, ($($marker,)*), S> for ($($handler,)*)
        where
            C: Sync,
            S: Sync,
            $($handler: Handler<C, $marker, S>,)*
        {
            async fn call(&self, cx: &Context<C>, state: &S) -> Outcome {
                let ($($handler,)*) = self;
                $(
                    let outcome = $handler.call(cx, state).await;
                    if !matches!(outcome, Outcome::Continue) {
                        return outcome;
                    }
                )*
                Outcome::Continue
            }
        }
    };
}

impl_sequence!((H1, M1), (H2, M2));
impl_sequence!((H1, M1), (H2, M2), (H3, M3));
impl_sequence!((H1, M1), (H2, M2), (H3, M3), (H4, M4));
