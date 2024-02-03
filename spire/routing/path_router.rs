use std::fmt;
use std::marker::PhantomData;

use crate::Label;

// use spire_core::Label;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct RouteId(u32);

pub struct PathRouter<S> {
    // Vec<Route>
    marker: PhantomData<S>,
    // routes: HashMap<RouteId, Vec<>>,
}

impl<S> PathRouter<S> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn route<L>(&mut self, label: L, handler: ())
    where
        L: Into<Label>,
    {
        todo!()
    }
}

impl<S> Clone for PathRouter<S> {
    fn clone(&self) -> Self {
        todo!()
    }
}

impl<S> fmt::Debug for PathRouter<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fallback").finish_non_exhaustive()
    }
}

impl<S> Default for PathRouter<S> {
    fn default() -> Self {
        Self {
            marker: PhantomData,
            // routes: HashMap::default(),
        }
    }
}
