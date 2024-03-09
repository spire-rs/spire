use std::convert::Infallible;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use pin_project_lite::pin_project;
use tower::util::{BoxCloneService, Oneshot};

use crate::context::{Context as Cx, Signal};

pin_project! {
    /// Response [`Future`] for [`Daemon`].
    ///
    /// [`Daemon`]: crate::process::Daemon
    pub struct DaemonFuture<B> {
        #[pin]
        kind: DaemonFutureKind<B>,
    }
}

/// Underlying [`Future`] type.
type Fut<B> = Oneshot<BoxCloneService<Cx<B>, Signal, Infallible>, Cx<B>>;

pin_project! {
    #[project = DaemonFutureKindProj]
    enum DaemonFutureKind<B> {
        Future { #[pin] future: Fut<B>, }
    }
}

impl<B> DaemonFuture<B> {
    pub fn new(future: Fut<B>) -> Self {
        let kind = DaemonFutureKind::Future { future };
        Self { kind }
    }
}

impl<F> fmt::Debug for DaemonFuture<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DaemonFuture").finish_non_exhaustive()
    }
}

impl<F> Future for DaemonFuture<F> {
    type Output = Signal;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let signal = match this.kind.project() {
            DaemonFutureKindProj::Future { future } => match future.poll(cx) {
                Poll::Ready(Ok(x)) => x,
                Poll::Ready(_) => unreachable!(),
                Poll::Pending => return Poll::Pending,
            },
        };

        Poll::Ready(signal)
    }
}
