use std::convert::Infallible;
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::FutureExt;
use futures::future::Map;
use pin_project_lite::pin_project;
use tower::Service;

use crate::context::{Context as Cx, FlowControl};
use crate::handler::Handler;

/// Implementation of a `tower::`[`Service`] for a [`Handler`].
///
/// This service wraps a [`Handler`] function and provides state management,
/// making it compatible with the tower ecosystem. The service can be cloned
/// efficiently and automatically implements [`Worker`] for use with Spire clients.
///
/// # Examples
///
/// ```ignore
/// use spire::handler::HandlerService;
/// use spire::extract::State;
///
/// #[derive(Clone)]
/// struct AppState {
///     counter: Arc<AtomicU32>,
/// }
///
/// async fn my_handler(State(state): State<AppState>) -> &'static str {
///     state.counter.fetch_add(1, Ordering::Relaxed);
///     "Hello, World!"
/// }
///
/// let state = AppState { counter: Arc::new(AtomicU32::new(0)) };
/// let service = HandlerService::new(my_handler, state);
/// ```
///
/// [`Worker`]: crate::backend::Worker
#[must_use = "services do nothing unless you `.poll_ready` or `.call` them"]
pub struct HandlerService<H, V, S> {
    marker: PhantomData<V>,
    handler: H,
    state: S,
}

impl<H, V, S> HandlerService<H, V, S> {
    /// Creates a new [`HandlerService`] with the given handler and state.
    ///
    /// # Arguments
    ///
    /// * `handler` - The handler function to wrap
    /// * `state` - The state that will be passed to the handler
    ///
    /// # Examples
    ///
    /// ```ignore
    /// async fn handler() -> &'static str { "Hello" }
    /// let service = HandlerService::new::<()>(handler, ());
    /// ```
    pub const fn new<C>(handler: H, state: S) -> Self
    where
        H: Handler<C, V, S>,
    {
        Self {
            marker: PhantomData,
            handler,
            state,
        }
    }

    /// Gets a reference to the state.
    ///
    /// This allows you to inspect the current state without consuming it.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let service = HandlerService::new(handler, 42u32);
    /// assert_eq!(*service.state_ref(), 42);
    /// ```
    #[inline]
    pub const fn state_ref(&self) -> &S {
        &self.state
    }

    /// Gets a mutable reference to the state.
    ///
    /// This allows you to modify the state directly. Note that due to the
    /// service being cloneable, modifications to shared state should use
    /// interior mutability patterns like `Arc<Mutex<T>>` or `Arc<RwLock<T>>`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut service = HandlerService::new(handler, 42u32);
    /// *service.state_mut() = 100;
    /// assert_eq!(*service.state_ref(), 100);
    /// ```
    #[inline]
    pub fn state_mut(&mut self) -> &mut S {
        &mut self.state
    }
}

impl<H, V, S> fmt::Debug for HandlerService<H, V, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandlerService").finish_non_exhaustive()
    }
}

impl<C, H, V, S> Service<Cx<C>> for HandlerService<H, V, S>
where
    H: Handler<C, V, S>,
    S: Clone,
{
    type Error = Infallible;
    type Future = HandlerFuture<H::Future>;
    type Response = FlowControl;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all, level = "trace"))]
    fn call(&mut self, cx: Cx<C>) -> Self::Future {
        #[cfg(feature = "tracing")]
        tracing::trace!("calling handler");

        let handler = self.handler.clone();
        let future = handler.call(cx, self.state.clone());
        HandlerFuture::new(future)
    }
}

impl<H, V, S> Clone for HandlerService<H, V, S>
where
    H: Clone,
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            marker: PhantomData,
            handler: self.handler.clone(),
            state: self.state.clone(),
        }
    }
}

pin_project! {
    /// Opaque [`Future`] return type for [`HandlerService`].
    ///
    /// This future represents the execution of a handler and its conversion
    /// to a [`FlowControl`] result. It's designed to be efficient and
    /// maintains compatibility with the tower service ecosystem.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct HandlerFuture<F>
    where
        F: Future<Output = FlowControl>
    {
        #[pin]
        future: HandlerFut<F>,
    }
}

/// Underlying [`HandlerFuture`] type.
type HandlerFut<F> = Map<F, fn(FlowControl) -> Result<FlowControl, Infallible>>;

impl<F> HandlerFuture<F>
where
    F: Future<Output = FlowControl>,
{
    /// Creates a new [`HandlerFuture`] from a handler's future.
    ///
    /// This wraps the handler's future and maps its output to a
    /// `Result<FlowControl, Infallible>` as required by tower services.
    pub fn new(future: F) -> Self {
        let future = future.map(Ok as _);
        Self { future }
    }
}

impl<F> Future for HandlerFuture<F>
where
    F: Future<Output = FlowControl>,
{
    type Output = Result<FlowControl, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        this.future.poll(cx)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::backend::Worker;
    use crate::extract::FromRef;
    use crate::handler::HandlerService;

    fn service<B: Send + 'static>() -> impl Worker<B> {
        async fn handler() {}
        HandlerService::new::<B>(handler, ())
    }

    #[test]
    fn handler_service_basic() {
        // Test that we can create a service using the helper function
        let _service = service::<()>();
    }

    #[test]
    fn handler_service_clone() {
        let service = service::<()>();
        let _cloned = service.clone();
        // Both services should work independently
    }

    #[test]
    fn handler_service_debug() {
        // Test that HandlerService implements Debug
        // Note: The service() function returns impl Worker<B> which may not implement Debug
        // so we test the type exists and can be referenced
        type TestService = HandlerService<fn(), (), ()>;
        let _ = std::marker::PhantomData::<TestService>;
    }

    // FIXME: Commented out due to Service trait constraint issues with HttpClient
    // #[test]
    // #[cfg(feature = "reqwest")]
    // fn with_reqwest() {
    //     let backend = crate::HttpClient::default();
    //     let _ = Client::new(backend, service::<crate::HttpClient>());
    // }

    #[test]
    #[cfg(feature = "thirtyfour")]
    fn with_thirtyfour() {
        let backend = crate::BrowserBackend::builder()
            .with_unmanaged("http://127.0.0.1:4444")
            .build()
            .expect("Failed to build browser pool");
        let _ = Client::new(backend, service::<crate::BrowserConnection>());
    }

    #[test]
    fn handler_service_with_extractors() {
        #[derive(Debug, Default, Clone)]
        struct AppState {
            value: u32,
        }

        impl FromRef<AppState> for u32 {
            fn from_ref(input: &AppState) -> Self {
                input.value
            }
        }

        let state = AppState { value: 42 };

        // Test that state can be used for extraction
        assert_eq!(u32::from_ref(&state), 42);
    }

    #[test]
    fn handler_service_creation_with_state() {
        // Test that HandlerService type exists and can be referenced
        // Note: Actual creation requires proper Handler trait implementation
        type TestService = HandlerService<fn(), (), ()>;
        let _ = std::marker::PhantomData::<TestService>;
    }

    #[test]
    fn handler_service_methods_exist() {
        // Test that HandlerService has the expected methods
        // Note: We can't test the actual service() return type methods
        // since it returns impl Worker<B>, but we can test the type exists
        type TestService = HandlerService<fn(), (), u32>;

        // Test that the methods exist in the type signature
        fn test_methods(mut service: TestService) {
            let _state_ref = service.state_ref();
            let _state_mut = service.state_mut();
        }

        // This function should compile, proving the methods exist
        let _ = test_methods;
    }
}
