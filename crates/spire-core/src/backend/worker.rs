//! Worker trait for processing scraping tasks.

use std::convert::Infallible;
use std::future::Future;

use tower::{Service, ServiceExt};

use crate::context::{Context, FlowControl};

/// Core trait for processing scraping tasks.
///
/// A `Worker` processes a [`Context`] (containing a request, client, and datasets)
/// and returns a [`FlowControl`] that controls execution flow. It is automatically
/// implemented for cloneable Tower services.
///
/// # Examples
///
/// ```no_run
/// use spire_core::backend::Worker;
/// use spire_core::context::{Context, FlowControl};
///
/// #[derive(Clone)]
/// struct MyWorker;
///
/// impl<C> Worker<C> for MyWorker {
///     async fn invoke(self, cx: Context<C>) -> FlowControl {
///         // Process the context
///         FlowControl::Continue
///     }
/// }
/// ```
///
/// [`Context`]: crate::context::Context
pub trait Worker<C>: Clone + Send + 'static {
    /// Processes the context and returns a flow control signal.
    ///
    /// ## Note
    ///
    /// This method consumes `self` due to current Tower service requirements.
    /// Future versions may use `&self` instead.
    fn invoke(self, cx: Context<C>) -> impl Future<Output = FlowControl>;
}

impl<S, C> Worker<C> for S
where
    S: Service<Context<C>, Response = FlowControl, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send + 'static,
    C: Send + 'static,
{
    #[inline]
    async fn invoke(self, cx: Context<C>) -> FlowControl {
        let mut this = self.clone();
        let ready = this.ready().await.expect("Worker should be infallible");
        ready.call(cx).await.expect("Worker should be infallible")
    }
}
