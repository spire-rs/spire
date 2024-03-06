use std::convert::Infallible;
use std::fmt;
use std::marker::PhantomData;

use tower::Service;

use crate::context::{Context, IntoSignal};
use crate::process::daemon::Process;

mod daemon;
mod metric;

pub struct Daemon<B> {
    inner: Process<()>,
    backend: PhantomData<B>,
}

impl<B> Daemon<B> {
    pub fn new<S>(svc: S) -> Self
    where
        S: Service<Context<B>, Error = Infallible>,
        S::Response: IntoSignal,
    {
        todo!()
    }
}

impl<B> Clone for Daemon<B> {
    fn clone(&self) -> Self {
        todo!()
    }
}

impl<B> fmt::Debug for Daemon<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Daemon").finish_non_exhaustive()
    }
}
