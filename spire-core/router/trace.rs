use std::marker::PhantomData;

use tower_layer::Layer;

use crate::router::NullRouter;

#[derive(Debug, Clone)]
pub struct TraceRouter<T = (), S = NullRouter<T>> {
    marker: PhantomData<T>,
    svc: S,
}

impl<T, S> TraceRouter<T, S> {
    pub fn new(svc: S) -> Self {
        Self {
            marker: PhantomData,
            svc,
        }
    }

    /// Consumes self and returns the inner service,
    pub fn into_inner(self) -> S {
        self.svc
    }

    /// Returns a reference to the inner service.
    pub fn get_ref(&self) -> &S {
        &self.svc
    }

    /// Returns a mutable reference to the inner service.
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.svc
    }
}

impl<T, S> Default for TraceRouter<T, S>
where
    S: Default,
{
    fn default() -> Self {
        Self::new(S::default())
    }
}

pub struct TraceRouterLayer {}

impl<T, S> Layer<S> for TraceRouterLayer {
    type Service = TraceRouter<T, S>;

    fn layer(&self, inner: S) -> Self::Service {
        TraceRouter::new(inner)
    }
}
