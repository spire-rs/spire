use std::convert::Infallible;
use std::fmt;
use std::marker::PhantomData;

pub struct Fallback<S, E = Infallible> {
    phantom: PhantomData<S>,
    phantom2: PhantomData<E>,
    handler: (),
}

impl<S, E> Fallback<S, E> {
    pub fn new(handler: ()) -> Self {
        Self {
            phantom: PhantomData,
            phantom2: PhantomData,
            handler,
        }
    }
}

impl<S> Clone for Fallback<S> {
    fn clone(&self) -> Self {
        todo!()
    }
}

impl<S> fmt::Debug for Fallback<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fallback").finish_non_exhaustive()
    }
}

impl<S> Default for Fallback<S> {
    fn default() -> Self {
        let handler = ();
        Self::new(handler)
    }
}
