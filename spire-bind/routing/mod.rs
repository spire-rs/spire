use std::fmt;

mod endpoint;

pub struct DynRouter {}

impl DynRouter {
    pub fn new() -> Self {
        todo!()
    }
}

impl Default for DynRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for DynRouter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DynRouter").finish_non_exhaustive()
    }
}
