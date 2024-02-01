use std::collections::HashMap;
use std::fmt;

use spire_core::collect::Label;

pub struct PathRouter<S> {
    // Vec<Route>
    routes: HashMap<String, S>,
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
            routes: HashMap::default(),
        }
    }
}
