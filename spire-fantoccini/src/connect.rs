use std::fmt;
use std::ops::{Deref, DerefMut};

use fantoccini::Client as WebClient;

/// Represents a connection to a browser instance.
///
/// This type wraps a Fantoccini client along with connection metadata.
/// It is used internally by the pool manager to track browser instances.
pub struct BrowserConnection {
    /// Unique identifier for this connection
    pub id: u64,
    /// The underlying Fantoccini WebDriver client
    pub client: WebClient,
}

impl BrowserConnection {
    // Connection management methods can be added here as needed
}

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
