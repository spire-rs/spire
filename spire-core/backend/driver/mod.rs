use std::fmt;
use std::future::Ready;
use std::task::{Context, Poll};

use deadpool::managed::Pool;
use tower::Service;

pub use client::BrowserClient;
pub use manager::BrowserManager;

use crate::{Error, Result};

mod client;
mod manager;
mod process;

/// Web-driver [`Backend`] built on top of [`fantoccini`] crate.
/// Uses [`BrowserClient`] as a [`Client`].
///
/// [`Backend`]: crate::backend::Backend
/// [`Client`]: crate::backend::Client
#[derive(Clone)]
pub struct BrowserPool {
    pool: Pool<BrowserManager>,
}

impl BrowserPool {
    /// Creates a new [`BrowserPool`].
    pub(crate) fn new(pool: Pool<BrowserManager>) -> Self {
        Self { pool }
    }

    pub(crate) async fn get(&self) -> Result<BrowserClient> {
        // BoxFuture::new()

        let inner = self.pool.get().await.unwrap(); // TODO.
        Ok(BrowserClient::new(inner))
    }

    /// Creates a new [`BrowserManager`].
    pub fn builder() -> BrowserManager {
        BrowserManager::new()
    }
}

impl fmt::Debug for BrowserPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Driver").finish_non_exhaustive()
    }
}

impl Service<()> for BrowserPool {
    type Response = BrowserClient;
    type Error = Error;
    type Future = Ready<Result<Self::Response, Self::Error>>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // TODO: Check for available browsers.
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: ()) -> Self::Future {
        // self.pool.get().await
        todo!()
    }
}

#[cfg(test)]
mod test {
    use crate::backend::BrowserManager;

    #[test]
    pub fn builder() {
        let manager = BrowserManager::default();
        let _ = manager.build();
    }
}
