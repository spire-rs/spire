use std::convert::Infallible;
use std::fmt;
use std::marker::PhantomData;

use tower::Service;

use crate::context::Context;
pub use crate::process::signal::{IntoSignal, Signal};

mod metric;
mod signal;

pub struct Process<B> {
    backend: PhantomData<B>,
}

struct Daemon {}

impl<B> Process<B> {
    pub fn new<S>(svc: S) -> Self
    where
        S: Service<Context<B>, Error = Infallible>,
        S::Response: IntoSignal,
    {
        todo!()
    }
}

impl<B> Clone for Process<B> {
    fn clone(&self) -> Self {
        todo!()
    }
}

impl<B> fmt::Debug for Process<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}
