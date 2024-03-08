use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use pin_project_lite::pin_project;
use tower::util::{BoxCloneService, Oneshot};

use spire_core::context::{Context as Cx, Signal};

pin_project! {
    /// Response [`Future`] for [`Route`].
    ///
    /// [`Route`]: crate::routing::Route
    pub struct RouteFuture<B, E> {
        #[pin]
        kind: RouteFutureKind<B, E>,
    }
}

/// Underlying [`Future`] type.
type Fut<B, E> = Oneshot<BoxCloneService<Cx<B>, Signal, E>, Cx<B>>;

pin_project! {
    #[project = RouteFutureKindProj]
    enum RouteFutureKind<B, E> {
        Future { #[pin] future: Fut<B, E>, },
        Signal { signal: Option<Signal>, },
    }
}

impl<B, E> RouteFuture<B, E> {
    /// Creates a new [` RouteFuture`].
    pub fn new(future: Fut<B, E>) -> Self {
        let kind = RouteFutureKind::Future { future };
        Self { kind }
    }
}

impl<B, E> fmt::Debug for RouteFuture<B, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RouteFuture").finish_non_exhaustive()
    }
}

impl<B, E> Future for RouteFuture<B, E> {
    type Output = Result<Signal, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let signal = match this.kind.project() {
            RouteFutureKindProj::Future { future } => match future.poll(cx) {
                Poll::Ready(Ok(sig)) => sig,
                Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                Poll::Pending => return Poll::Pending,
            },
            RouteFutureKindProj::Signal { signal } => signal
                .take()
                .expect("future should not be polled after completion"),
        };

        Poll::Ready(Ok(signal))
    }
}
