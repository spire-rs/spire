use std::convert::Infallible;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use tower::Service;

use crate::context::{Context, IntoSignal};

mod future;
mod metric;

pub struct Daemon<B, S> {
    inner: Arc<DaemonInner<B, S>>,
}

struct DaemonInner<B, S> {
    inner: S,
    backend: PhantomData<B>,
}

impl<B, S> Daemon<B, S> {
    pub fn new(svc: S) -> Self
    where
        S: Service<Context<B>, Error = Infallible>,
        S::Response: IntoSignal,
    {
        todo!()
    }
}

impl<B, S> Clone for Daemon<B, S> {
    fn clone(&self) -> Self {
        let inner = self.inner.clone();
        Self { inner }
    }
}

impl<B, S> fmt::Debug for Daemon<B, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Daemon").finish_non_exhaustive()
    }
}
