use std::convert::Infallible;
use std::fmt;
use std::future::{ready, Ready};
use std::task::{Context, Poll};

use futures::future::BoxFuture;
use futures::FutureExt;
use tower::Service;

use crate::context::{Body, IntoSignal, Signal};
use crate::context::{Context as Cx, Request, Response};
use crate::{Error, Result};

/// No-op `tower::`[`Service`] used for testing and debugging.
///
/// Supports [`Backend`], [`Client`] and [`Worker`].
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
    /// Creates a new [`Noop`] with an `always` rule.
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
    type Response = Self;
    type Error = Error;
    type Future = Ready<Result<Self, Error>>;

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
    type Response = Response;
    type Error = Error;
    type Future = Ready<Result<Response, Error>>;

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
    type Response = Signal;
    type Error = Infallible;
    type Future = BoxFuture<'static, Result<Signal, Infallible>>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, cx: Cx<C>) -> Self::Future {
        if let Some(always) = self.always {
            let signal = if always {
                Signal::Continue
            } else {
                Signal::Skip
            };

            return ready(Ok(signal)).boxed();
        }

        let fut = async move {
            let response = cx.resolve().await;
            Ok(response.map_or_else(IntoSignal::into_signal, |_| Signal::Continue))
        };

        fut.boxed()
    }
}
