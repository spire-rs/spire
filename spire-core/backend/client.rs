use std::fmt;

pub struct Client {}

impl Client {
    pub fn new() -> Self {
        todo!()
    }
}

impl Clone for Client {
    fn clone(&self) -> Self {
        todo!()
    }
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client").finish_non_exhaustive()
    }
}

// HttpClient
