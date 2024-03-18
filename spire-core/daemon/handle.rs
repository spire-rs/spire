use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::future::BoxFuture;

use crate::Result;

/// TODO.
/// is used to wait until the [`RequestQueue`] is empty.
///
/// [`RequestQueue`]: crate::context::RequestQueue
pub struct DaemonHandle {
    // TODO: tokio join handle?
    fut: BoxFuture<'static, Result<usize>>,
}

impl DaemonHandle {
    /// Creates a new [`DaemonHandle`].
    pub(crate) fn new<F>(fut: F) -> Self
    where
        F: Future<Output = Result<usize>>,
    {
        todo!()
    }
}

impl Future for DaemonHandle {
    type Output = Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        todo!()
    }
}
