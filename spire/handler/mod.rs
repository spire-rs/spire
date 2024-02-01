use std::future::Future;
use std::pin::Pin;

use spire_core::all_the_tuples;
use spire_core::collect::HandlerContext;

use crate::extract::{FromContext, FromContextParts};
pub use crate::handler::control::{ControlFlow, IntoFlow};
pub use crate::handler::service::HandlerService;

mod control;
mod service;

pub trait Handler<T, S>: Clone + Send + Sized + 'static {
    type Future: Future<Output = ControlFlow>;

    /// Call the handler with the given context.
    fn call(self, cx: HandlerContext, state: S) -> Self::Future;

    /// Convert the handler into a [`Service`] by providing the state
    ///
    /// [`Service`]: tower_service::Service
    fn with_state(self, state: S) -> HandlerService<Self, T, S> {
        todo!()
    }
}

impl<S, F, Fut, Ret> Handler<((),), S> for F
where
    F: FnOnce() -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Ret> + Send,
    Ret: IntoFlow,
{
    type Future = Pin<Box<dyn Future<Output = ControlFlow> + Send>>;

    fn call(self, _cx: HandlerContext, _state: S) -> Self::Future {
        Box::pin(async move { self().await.into_flow() })
    }
}

/// From [`axum`]/handler/mod.rs
///
/// [`axum`]: https://github.com/tokio-rs/axum
macro_rules! impl_handler {
    (
        [$($ty:ident),*], $last:ident
    ) => {
        impl<S, F, Fut, Ret, M, $($ty,)* $last> Handler<(M, $($ty,)* $last,), S> for F
        where
            S: Send + Sync + 'static,
            F: FnOnce($($ty,)* $last,) -> Fut + Clone + Send + 'static,
            Fut: Future<Output = Ret> + Send,
            Ret: IntoFlow,
            $( $ty: FromContextParts<S> + Send, )*
            $last: FromContext<S, M> + Send,
        {
            type Future = Pin<Box<dyn Future<Output = ControlFlow> + Send>>;

            fn call(self, cx: HandlerContext, state: S) -> Self::Future {
                Box::pin(async move {
                    $(
                        let $ty = match $ty::from_context_parts(&cx, &state).await {
                            Ok(value) => value,
                            Err(rejection) => return rejection.into_flow(),
                        };
                    )*

                    let $last = match $last::from_context(cx, &state).await {
                        Ok(value) => value,
                        Err(rejection) => return rejection.into_flow(),
                    };

                    let res = self($($ty,)* $last,).await;
                    res.into_flow()
                })
            }
        }
    };
}

all_the_tuples!(impl_handler);
