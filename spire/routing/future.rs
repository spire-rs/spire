use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use pin_project_lite::pin_project;
use tower::util::{BoxCloneService, Oneshot};

use crate::context::{Context as Cx, Signal};

pin_project! {
    /// Response [`Future`] for [`Route`].
    ///
    /// [`Route`]: crate::routing::Route
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct RouteFuture<C, E> {
        #[pin] kind: RouteFutureKind<C, E>,
    }
}

/// Underlying [`Future`] type.
type Fut<C, E> = Oneshot<BoxCloneService<Cx<C>, Signal, E>, Cx<C>>;

pin_project! {
    #[project = RouteFutureKindProj]
    enum RouteFutureKind<C, E> {
        Future { #[pin] fut: Fut<C, E>, },
        Signal { signal: Option<Signal>, },
    }
}

impl<C, E> RouteFuture<C, E> {
    /// Creates a new [` RouteFuture`].
    pub(crate) const fn new(fut: Fut<C, E>) -> Self {
        let kind = RouteFutureKind::Future { fut };
        Self { kind }
    }
}

impl<C, E> fmt::Debug for RouteFuture<C, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RouteFuture").finish_non_exhaustive()
    }
}

impl<C, E> Future for RouteFuture<C, E> {
    type Output = Result<Signal, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let signal = match this.kind.project() {
            RouteFutureKindProj::Future { fut } => match fut.poll(cx) {
                Poll::Ready(Ok(x)) => x,
                Poll::Ready(Err(x)) => return Poll::Ready(Err(x)),
                Poll::Pending => return Poll::Pending,
            },
            RouteFutureKindProj::Signal { signal } => signal
                .take()
                .expect("future should not be polled after completion"),
        };

        Poll::Ready(Ok(signal))
    }
}
