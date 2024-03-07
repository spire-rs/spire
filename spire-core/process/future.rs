use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::context::Signal;

pub struct DaemonFuture<F> {
    future: F,
}

impl<F> DaemonFuture<F> {
    pub fn new(future: F) -> Self {
        Self { future }
    }
}

impl<F> fmt::Debug for DaemonFuture<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}
impl<F> Future for DaemonFuture<F> {
    type Output = Signal;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        todo!()
    }
}
