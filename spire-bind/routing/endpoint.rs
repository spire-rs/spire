use std::fmt;
use std::fmt::Pointer;

pub struct DynEndpoint {}

impl DynEndpoint {
    pub fn new() -> Self {
        todo!()
    }
}

impl Default for DynEndpoint {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for DynEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DynEndpoint").finish_non_exhaustive()
    }
}
