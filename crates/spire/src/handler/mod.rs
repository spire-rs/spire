//! Handlers: async functions of extractors that handle a request.

use std::future::Future;

use crate::Context;
use crate::extract::{FromContext, FromContextRef};
use crate::worker::{IntoOutcome, Outcome};

/// An async function that handles a request, usually not implemented by hand.
///
/// It is implemented for async functions of up to 16 [extractors] whose
/// return type implements [`IntoOutcome`], such as `()`, `Result<(), Error>`
/// or [`Outcome`]. All arguments but the last must implement
/// [`FromContextRef`]; the last may also implement [`FromContext`].
///
/// ```
/// use spire::extract::Text;
/// use spire::request::Tag;
///
/// async fn handler(tag: Tag, Text(body): Text) -> spire::Result<()> {
///     println!("{tag}: {} bytes", body.len());
///     Ok(())
/// }
/// ```
///
/// [extractors]: crate::extract
pub trait Handler<C, M, S>: Clone + Send + Sync + 'static {
    /// Handles the request in `cx`.
    fn call(self, cx: Context<C>, state: S) -> impl Future<Output = Outcome> + Send + 'static;
}

impl<C, S, F, Fut> Handler<C, ((),), S> for F
where
    C: Send + 'static,
    S: Send + 'static,
    F: FnOnce() -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output: IntoOutcome> + Send + 'static,
{
    async fn call(self, _cx: Context<C>, _state: S) -> Outcome {
        self().await.into_outcome()
    }
}

macro_rules! impl_handler {
    ([$($ty:ident),*], $last:ident) => {
        #[expect(non_snake_case, reason = "extractors are bound to their type parameters' names")]
        impl<C, S, F, Fut, M, $($ty,)* $last> Handler<C, (M, $($ty,)* $last,), S> for F
        where
            C: Send + Sync + 'static,
            S: Send + Sync + 'static,
            F: FnOnce($($ty,)* $last) -> Fut + Clone + Send + Sync + 'static,
            Fut: Future<Output: IntoOutcome> + Send + 'static,
            M: 'static,
            $($ty: FromContextRef<C, S> + Send + 'static,)*
            $last: FromContext<C, S, M> + Send + 'static,
        {
            async fn call(self, cx: Context<C>, state: S) -> Outcome {
                $(
                    let $ty = match $ty::from_context_ref(&cx, &state).await {
                        Ok(value) => value,
                        Err(rejection) => return rejection.into_outcome(),
                    };
                )*
                let $last = match $last::from_context(cx, &state).await {
                    Ok(value) => value,
                    Err(rejection) => return rejection.into_outcome(),
                };
                self($($ty,)* $last).await.into_outcome()
            }
        }
    };
}

impl_handler!([], T1);
impl_handler!([T1], T2);
impl_handler!([T1, T2], T3);
impl_handler!([T1, T2, T3], T4);
impl_handler!([T1, T2, T3, T4], T5);
impl_handler!([T1, T2, T3, T4, T5], T6);
impl_handler!([T1, T2, T3, T4, T5, T6], T7);
impl_handler!([T1, T2, T3, T4, T5, T6, T7], T8);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8], T9);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9], T10);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10], T11);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11], T12);
impl_handler!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12], T13);
impl_handler!(
    [T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13],
    T14
);
impl_handler!(
    [T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14],
    T15
);
impl_handler!(
    [
        T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15
    ],
    T16
);
