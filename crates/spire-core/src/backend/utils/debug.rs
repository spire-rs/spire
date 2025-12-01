use std::convert::Infallible;
use std::fmt;
use std::future::{Ready, ready};
use std::task::{Context, Poll};

use futures::FutureExt;
use futures::future::BoxFuture;
use tower::Service;

use crate::context::{Body, Context as Cx, FlowControl, IntoFlowControl, Request, Response};
use crate::{Error, Result};

/// No-op `tower::`[`Service`] used for testing and debugging.
///
/// `Noop` implements all three core traits ([`Backend`], [`Client`], and [`Worker`])
/// with minimal behavior, making it useful for:
///
/// - Unit testing components without real backends
/// - Prototyping scraping logic before implementing full backends
/// - Debugging worker behavior in isolation
///
/// # Behavior
///
/// - **As Backend**: Returns a clone of itself as the client
/// - **As Client**: Returns an empty response with default body
/// - **As Worker**: Can be configured to always continue, skip, or resolve the request
///
/// # Examples
///
/// ## Basic Testing
///
/// ```no_run
/// use spire_core::backend::utils::Noop;
/// use spire_core::Client;
///
/// let backend = Noop::default();
/// let worker = Noop::default();
/// let client = Client::new(backend, worker);
/// ```
///
/// ## Controlling Worker Behavior
///
/// ```no_run
/// use spire_core::backend::utils::Noop;
///
/// // Always continue without resolving requests
/// let worker = Noop::new(Some(true));
///
/// // Always skip
/// let worker = Noop::new(Some(false));
///
/// // Resolve requests and continue on success
/// let worker = Noop::new(None);
/// ```
///
/// [`Backend`]: crate::backend::Backend
/// [`Client`]: crate::backend::Client
/// [`Worker`]: crate::backend::Worker
#[derive(Clone, Default)]
#[must_use = "services do nothing unless you `.poll_ready` or `.call` them"]
pub struct Noop {
    always: Option<bool>,
}

impl Noop {
    /// Creates a new [`Noop`] with an `always` rule for worker behavior.
    ///
    /// # Parameters
    ///
    /// - `Some(true)` - Always returns [`FlowControl::Continue`] without resolving requests
    /// - `Some(false)` - Always returns [`FlowControl::Skip`] without resolving requests
    /// - `None` - Resolves requests and returns [`FlowControl::Continue`] on success
    ///
    /// [`FlowControl::Continue`]: crate::context::FlowControl::Continue
    /// [`FlowControl::Skip`]: crate::context::FlowControl::Skip
    pub const fn new(always: Option<bool>) -> Self {
        Self { always }
    }
}

impl fmt::Debug for Noop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Noop").finish_non_exhaustive()
    }
}

impl Service<()> for Noop {
    type Error = Error;
    type Future = Ready<Result<Self, Error>>;
    type Response = Self;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, _req: ()) -> Self::Future {
        ready(Ok(self.clone()))
    }
}

impl Service<Request> for Noop {
    type Error = Error;
    type Future = Ready<Result<Response, Error>>;
    type Response = Response;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, _req: Request) -> Self::Future {
        ready(Ok(Response::new(Body::default())))
    }
}

impl<C> Service<Cx<C>> for Noop
where
    C: Service<Request, Response = Response, Error = Error> + Send + 'static,
    C::Future: Send,
{
    type Error = Infallible;
    type Future = BoxFuture<'static, Result<FlowControl, Infallible>>;
    type Response = FlowControl;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, cx: Cx<C>) -> Self::Future {
        if let Some(always) = self.always {
            let flow_control = if always {
                FlowControl::Continue
            } else {
                FlowControl::Skip
            };

            return ready(Ok(flow_control)).boxed();
        }

        let fut = async move {
            let response = cx.resolve().await;
            Ok(
                response.map_or_else(IntoFlowControl::into_flow_control, |_| {
                    FlowControl::Continue
                }),
            )
        };

        fut.boxed()
    }
}
