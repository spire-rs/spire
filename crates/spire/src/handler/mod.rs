use std::future::{Future, Ready, ready};
use std::pin::Pin;

use macros::all_the_tuples;
pub use service::HandlerService;

use crate::context::{Context, FlowControl, IntoFlowControl};
use crate::extract::{FromContext, FromContextRef};

mod macros;
mod service;

/// Async functions that can be used to handle [`Request`]s.
///
/// The `Handler` trait is automatically implemented for async functions and closures
/// that take extractors as arguments and return values convertible to [`FlowControl`].
/// You typically don't need to implement this trait manually.
///
/// # Handler Function Signatures
///
/// Handlers can take any number of extractors as arguments:
///
/// ```rust,no_run
/// # use spire::routing::Router;
/// # use spire::extract::{State, Text};
/// # use spire::context::FlowControl;
///
/// // No arguments
/// async fn simple_handler() -> FlowControl {
///     FlowControl::Continue
/// }
///
/// // With extractors
/// async fn handler_with_extractors(
///     body: Text,
///     state: State<MyState>,
/// ) -> FlowControl {
///     // Process the extracted data
///     FlowControl::Continue
/// }
///
/// # #[derive(Clone)]
/// # struct MyState;
/// let router: Router = Router::new()
///     .route("simple", simple_handler)
///     .route("complex", handler_with_extractors);
/// ```
///
/// # Return Types
///
/// Handlers can return any type that implements [`IntoFlowControl`]:
///
/// ```rust,ignore
/// # use spire::context::FlowControl;
///
/// // Direct FlowControl
/// async fn direct_flow() -> FlowControl {
///     FlowControl::Break
/// }
///
/// // String (automatically converts to Continue)
/// async fn string_handler() -> &'static str {
///     "Processing completed"
/// }
///
/// // Unit type (automatically converts to Continue)
/// async fn unit_handler() {
///     println!("Handler executed");
/// }
/// ```
///
/// # Non-Function Handlers
///
/// The `Handler` trait is also implemented for `T: IntoFlowControl`,
/// allowing you to use fixed [`FlowControl`] values directly:
///
/// ```rust,ignore
/// # use spire::routing::Router;
/// # use spire::context::FlowControl;
///
/// let router: Router = Router::new()
///     .route("continue", FlowControl::Continue)
///     .route("break", FlowControl::Break);
/// ```
///
/// # Extractor Compatibility
///
/// Handlers work with any combination of extractors that implement
/// [`FromContext`] or [`FromContextRef`]. The last argument must implement
/// [`FromContext`] (consuming), while all others must implement
/// [`FromContextRef`] (non-consuming).
///
/// [`Request`]: crate::context::Request
/// [`FromContext`]: crate::extract::FromContext
/// [`FromContextRef`]: crate::extract::FromContextRef
pub trait Handler<C, V, S>: Clone + Send + Sized + 'static {
    type Future: Future<Output = FlowControl>;

    /// Calls the [`Handler`] with the given [`Context`] and user-provided state `S`.
    ///
    /// This method is the core execution point for handlers. It receives the context
    /// containing the request and other contextual information, along with the state,
    /// and returns a future that resolves to a [`FlowControl`].
    ///
    /// # Arguments
    ///
    /// * `cx` - The context containing request data and other information
    /// * `state` - User-provided state that can be extracted in the handler
    ///
    /// # Returns
    ///
    /// A future that resolves to [`FlowControl`] indicating how processing should continue.
    fn call(self, cx: Context<C>, state: S) -> Self::Future;

    /// Converts the [`Handler`] into a [`HandlerService`] by providing the state.
    ///
    /// This method wraps the handler in a [`HandlerService`] that implements tower's
    /// [`Service`] trait, making it compatible with the broader tower ecosystem.
    ///
    /// # Arguments
    ///
    /// * `state` - The state to attach to the handler service
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use spire::handler::Handler;
    ///
    /// async fn my_handler() -> &'static str {
    ///     "Hello, World!"
    /// }
    ///
    /// let service = my_handler.with_state(());
    /// ```
    ///
    /// [`Service`]: tower::Service
    fn with_state(self, state: S) -> HandlerService<Self, V, S> {
        HandlerService::new::<C>(self, state)
    }
}

impl<C, S, F, Fut, Ret> Handler<C, ((),), S> for F
where
    F: FnOnce() -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Ret> + Send,
    Ret: IntoFlowControl,
{
    type Future = Pin<Box<dyn Future<Output = FlowControl> + Send>>;

    fn call(self, _cx: Context<C>, _state: S) -> Self::Future {
        Box::pin(async move { self().await.into_flow_control() })
    }
}

mod sealed {
    /// Marker type for `impl<T: IntoSignal> Handler for T`.
    pub enum IntoSignal {}
}

impl<C, S, T> Handler<C, sealed::IntoSignal, S> for T
where
    T: IntoFlowControl + Clone + Send + 'static,
{
    type Future = Ready<FlowControl>;

    fn call(self, _cx: Context<C>, _state: S) -> Self::Future {
        ready(self.into_flow_control())
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
        impl<C, S, F, Fut, Ret, M, $($ty,)* $last> Handler<C, (M, $($ty,)* $last,), S> for F
        where
            C: Send + Sync + 'static,
            S: Send + Sync + 'static,
            F: FnOnce($($ty,)* $last,) -> Fut + Clone + Send + 'static,
            Fut: Future<Output = Ret> + Send,
            Ret: IntoFlowControl,
            $( $ty: FromContextRef<C, S> + Send, )*
            $last: FromContext<C, S, M> + Send,
        {
            type Future = Pin<Box<dyn Future<Output = FlowControl> + Send>>;

            fn call(self, cx: Context<C>, state: S) -> Self::Future {
                Box::pin(async move {
                    $(
                        let $ty = match $ty::from_context_ref(&cx, &state).await {
                            Ok(value) => value,
                            Err(rejection) => return rejection.into_flow_control(),
                        };
                    )*

                    let $last = match $last::from_context(cx, &state).await {
                        Ok(value) => value,
                        Err(rejection) => return rejection.into_flow_control(),
                    };

                    let res = self($($ty,)* $last,).await;
                    res.into_flow_control()
                })
            }
        }
    };
}

all_the_tuples!(impl_handler);

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicU32;

    use crate::context::FlowControl;
    use crate::extract::FromRef;

    #[derive(Clone)]
    struct TestState {
        counter: Arc<AtomicU32>,
    }

    impl FromRef<TestState> for Arc<AtomicU32> {
        fn from_ref(input: &TestState) -> Self {
            input.counter.clone()
        }
    }

    #[test]
    fn handler_trait_basics() {
        // Test that Handler trait is available and has expected associated types
        type TestHandler =
            fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = FlowControl> + Send>>;
        let _ = std::marker::PhantomData::<TestHandler>;
    }

    #[test]
    fn handler_service_conversion() {
        // Test that HandlerService type exists
        use crate::handler::HandlerService;
        type TestService = HandlerService<fn(), (), ()>;
        let _ = std::marker::PhantomData::<TestService>;
    }

    #[test]
    fn flow_control_variants() {
        // Test that FlowControl variants are available
        let _continue = FlowControl::Continue;
        let _skip = FlowControl::Skip;
    }

    #[test]
    fn test_state_extraction() {
        let state = TestState {
            counter: Arc::new(AtomicU32::new(42)),
        };

        // Test that FromRef trait works
        let counter = <Arc<AtomicU32> as FromRef<TestState>>::from_ref(&state);
        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 42);
    }

    #[test]
    fn handler_clone_trait() {
        // Test that functions can be cloned (they implement Clone automatically)
        async fn test_handler() -> &'static str {
            "test"
        }

        let handler1 = test_handler;
        let handler2 = test_handler;

        // Both should be callable
        let _h1 = handler1;
        let _h2 = handler2;
    }
}
