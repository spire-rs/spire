use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use pin_project_lite::pin_project;
use tower::util::{BoxCloneService, Oneshot};

use spire_core::context::Context as Cx;
use spire_core::context::Signal;

pin_project! {
    /// Response [`Future`] for [`Route`].
    ///
    /// [`Route`]: crate::routing::Route
    pub struct RouteFuture<B, E> {
        #[pin]
        kind: RouteFutureKind<B, E>,
    }
}

pin_project! {
    #[project = RouteFutureKindProj]
    enum RouteFutureKind<B, E> {
        Future {
            #[pin]
            future: Oneshot<BoxCloneService<Cx<B>, Signal, E>, Cx<B>>
        },
        Signal { signal: Option<Signal> },
    }
}

impl<B, E> RouteFuture<B, E> {
    /// Creates a new [` RouteFuture`].
    pub fn new(future: Oneshot<BoxCloneService<Cx<B>, Signal, E>, Cx<B>>) -> Self {
        let kind = RouteFutureKind::Future { future };
        Self { kind }
    }
}

impl<B, E> Future for RouteFuture<B, E> {
    type Output = Result<Signal, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let sig = match this.kind.project() {
            RouteFutureKindProj::Future { future } => match future.poll(cx) {
                Poll::Ready(Ok(sig)) => sig,
                Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                Poll::Pending => return Poll::Pending,
            },
            RouteFutureKindProj::Signal { signal } => signal
                .take()
                .expect("future should not be polled after completion"),
        };

        Poll::Ready(Ok(sig))
    }
}
