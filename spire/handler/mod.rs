use std::future::{Future, ready, Ready};
use std::pin::Pin;

use macros::all_the_tuples;
pub use service::HandlerService;
use spire_core::context::{Context, IntoSignal, Signal};

use crate::extract::{FromContext, FromContextRef};

mod macros;
mod service;

/// Trait for async functions that can be used to handle [`Request`]s.
///
/// You shouldn't need to depend on this trait directly. It is automatically
/// implemented to closures of the right types.
///
/// ```rust
/// # use spire::routing::Router;
/// # use spire_core::context::Tag;
///
/// async fn handler() {}
///
/// let router: Router = Router::new()
///     .route(Tag::default(), handler);
/// ```
///
/// ### Handlers that aren't functions
///
/// The `Handler` trait is also implemented for `T: IntoSignal`.
/// That allows easily returning fixed [`Signal`] for routes:
///
/// ```rust
/// # use spire::routing::Router;
/// # use spire_core::context::{Tag, Signal};
///
/// let router: Router = Router::new()
///     .route(Tag::default(), Signal::Continue);
/// ```
///
/// [`Request`]: crate::context::Request
pub trait Handler<B, V, S>: Clone + Send + Sized + 'static {
    type Future: Future<Output = Signal>;

    /// Calls the [`Handler`] with the given [`Context`] and user-provided state `S`.
    fn call(self, cx: Context<B>, state: S) -> Self::Future;

    /// Converts the [`Handler`] into a [`Service`] by providing the state.
    ///
    /// [`Service`]: tower::Service
    fn with_state(self, state: S) -> HandlerService<Self, V, S> {
        HandlerService::new(self, state)
    }
}

impl<B, S, F, Fut, Ret> Handler<B, ((),), S> for F
where
    F: FnOnce() -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Ret> + Send,
    Ret: IntoSignal,
{
    type Future = Pin<Box<dyn Future<Output = Signal> + Send>>;

    fn call(self, _cx: Context<B>, _state: S) -> Self::Future {
        Box::pin(async move { self().await.into_signal() })
    }
}

mod sealed {
    /// Marker type for `impl<T: IntoSignal> Handler for T`.
    pub enum IntoSignal {}
}

impl<B, S, T> Handler<B, sealed::IntoSignal, S> for T
where
    T: IntoSignal + Clone + Send + 'static,
{
    type Future = Ready<Signal>;

    fn call(self, _cx: Context<B>, _state: S) -> Self::Future {
        ready(self.into_signal())
    }
}

/// Forked from [`axum`]`::handler::impl_handler`.
///
/// [`axum`]: https://github.com/tokio-rs/axum
macro_rules! impl_handler {
    (
        [$($ty:ident),*], $last:ident
    ) => {
        #[allow(non_snake_case)]
        impl<B, S, F, Fut, Ret, M, $($ty,)* $last> Handler<B, (M, $($ty,)* $last,), S> for F
        where
            B: Send + Sync + 'static,
            S: Send + Sync + 'static,
            F: FnOnce($($ty,)* $last,) -> Fut + Clone + Send + 'static,
            Fut: Future<Output = Ret> + Send,
            Ret: IntoSignal,
            $( $ty: FromContextRef<B, S> + Send, )*
            $last: FromContext<B,S, M> + Send,
        {
            type Future = Pin<Box<dyn Future<Output = Signal> + Send>>;

            fn call(self, cx: Context<B>, state: S) -> Self::Future {
                Box::pin(async move {
                    $(
                        let $ty = match $ty::from_context_parts(&cx, &state).await {
                            Ok(value) => value,
                            Err(rejection) => return rejection.into_signal(),
                        };
                    )*

                    let $last = match $last::from_context(cx, &state).await {
                        Ok(value) => value,
                        Err(rejection) => return rejection.into_signal(),
                    };

                    let res = self($($ty,)* $last,).await;
                    res.into_signal()
                })
            }
        }
    };
}

all_the_tuples!(impl_handler);
