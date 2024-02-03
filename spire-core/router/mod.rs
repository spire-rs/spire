use std::marker::PhantomData;

#[cfg(feature = "tracing")]
pub use trace::TraceRouter;

#[cfg(feature = "tracing")]
mod trace;

#[derive(Debug, Clone)]
pub struct NullRouter<T> {
    marker: PhantomData<T>,
}

impl<T> NullRouter<T> {
    pub fn new() -> Self {
        Self {
            marker: PhantomData,
        }
    }
}

impl<T> Default for NullRouter<T> {
    fn default() -> Self {
        Self::new()
    }
}
