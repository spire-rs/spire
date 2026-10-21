//! Type erasure for [`Frontier`]s shared by the crawler and its handlers.

use futures::FutureExt;
use futures::future::BoxFuture;

use crate::Result;
use crate::frontier::seed::Seed;
use crate::frontier::{Ack, Admission, Frontier, Next, Report};

/// An object-safe [`Frontier`].
pub(crate) trait DynFrontier: Send + Sync + 'static {
    fn push(&self, seed: Seed) -> BoxFuture<'_, Result<Admission>>;
    fn pop(&self) -> BoxFuture<'_, Result<Next>>;
    fn report(&self, report: Report) -> BoxFuture<'_, Result<Ack>>;
    fn size_hint(&self) -> Option<usize>;
    fn snapshot(&self) -> Option<Vec<Seed>>;
}

impl<F: Frontier> DynFrontier for F {
    fn push(&self, seed: Seed) -> BoxFuture<'_, Result<Admission>> {
        Frontier::push(self, seed).boxed()
    }

    fn pop(&self) -> BoxFuture<'_, Result<Next>> {
        Frontier::pop(self).boxed()
    }

    fn report(&self, report: Report) -> BoxFuture<'_, Result<Ack>> {
        Frontier::report(self, report).boxed()
    }

    fn size_hint(&self) -> Option<usize> {
        Frontier::size_hint(self)
    }

    fn snapshot(&self) -> Option<Vec<Seed>> {
        Frontier::snapshot(self)
    }
}
