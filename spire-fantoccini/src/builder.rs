use std::fmt;

use deadpool::managed::Pool;

use crate::manager::BrowserManager;
use crate::pool::BrowserPool;

/// Builder for configuring and creating a [`BrowserPool`].
///
/// Allows adding WebDriver connections (managed or unmanaged) before
/// constructing the final browser pool.
///
/// # Examples
///
/// ```ignore
/// use spire_fantoccini::BrowserPool;
///
/// let pool = BrowserPool::builder()
///     .with_unmanaged("127.0.0.1:4444")
///     .with_unmanaged("127.0.0.1:4445")
///     .build();
/// ```
#[must_use]
#[derive(Default)]
pub struct BrowserBuilder {
    connections: Vec<String>,
}

impl BrowserBuilder {
    /// Creates a new [`BrowserBuilder`].
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an unmanaged WebDriver connection to the pool.
    ///
    /// The connection points to an already-running WebDriver server
    /// (e.g., Selenium Grid, ChromeDriver, GeckoDriver).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_fantoccini::BrowserPool;
    ///
    /// let pool = BrowserPool::builder()
    ///     .with_unmanaged("127.0.0.1:4444")
    ///     .build();
    /// ```
    #[inline]
    pub fn with_unmanaged(mut self, addr: impl AsRef<str>) -> Self {
        self.connections.push(addr.as_ref().to_owned());
        self
    }

    /// Adds a managed WebDriver process to the pool.
    ///
    /// ## Note
    ///
    /// This functionality is not yet implemented and will panic.
    #[inline]
    pub fn with_managed(self) -> Self {
        todo!("with_managed is not yet implemented")
    }

    /// Constructs the [`BrowserPool`] from this builder.
    ///
    /// Creates a connection pool with a maximum size equal to the number
    /// of connections added.
    ///
    /// # Panics
    ///
    /// Panics if the pool cannot be built (e.g., runtime requirements not met).
    pub fn build(self) -> BrowserPool {
        let conns = self.connections.len();
        let manager = BrowserManager::new();
        // TODO: manager.with(connections).
        let pool = Pool::builder(manager).max_size(conns).build();
        pool.expect("failed to build browser pool").into()
    }
}

impl fmt::Debug for BrowserBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserBuilder")
            .field("connections", &self.connections)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod test {
    use crate::BrowserPool;

    #[test]
    fn with_unmanaged() {
        let _ = BrowserPool::builder()
            .with_unmanaged("127.0.0.1:4444")
            .with_unmanaged("127.0.0.1:4445")
            .build();
    }

    #[test]
    fn with_managed() {
        let _ = BrowserPool::builder()
            // .with_managed(Firefox::default())
            // .with_managed(Chrome::default())
            .build();
    }
}
