use std::fmt;

use deadpool::managed::Pool;

use crate::backend::driver::BrowserManager;
use crate::backend::BrowserPool;

/// [`BrowserPool`] builder.
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

    /// Adds an unmanaged `WebDriver` connection to the pool.
    #[inline]
    pub fn with_unmanaged(mut self, addr: impl AsRef<str>) -> Self {
        self.connections.push(addr.as_ref().to_owned());
        self
    }

    /// Adds a managed `WebDriver` process to the pool.
    #[inline]
    pub fn with_managed(self) -> Self {
        todo!()
    }

    /// Constructs a new [`BrowserPool`].
    pub fn build(self) -> BrowserPool {
        let conns = self.connections.len();
        let manager = BrowserManager::new();
        // TODO: manager.with().
        let pool = Pool::builder(manager).max_size(conns).build();
        pool.expect("should not require runtime").into()
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
    use crate::backend::BrowserPool;

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
