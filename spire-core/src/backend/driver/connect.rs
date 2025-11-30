use std::fmt;
use std::ops::{Deref, DerefMut};

use fantoccini::Client as WebClient;

/// TODO.
pub struct BrowserConnection {
    pub id: u64,
    pub client: WebClient,
}

impl BrowserConnection {}

impl fmt::Debug for BrowserConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserConnection")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl Deref for BrowserConnection {
    type Target = WebClient;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl DerefMut for BrowserConnection {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.client
    }
}
