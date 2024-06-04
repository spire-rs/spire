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
#[must_use]
#[derive(Clone)]
pub struct BrowserPool {
    pool: Pool<BrowserManager>,
}

impl BrowserPool {
    pub(crate) async fn get(&self) -> Result<BrowserClient> {
        // BoxFuture::new()

        let inner = self.pool.get().await.unwrap(); // TODO.
        Ok(inner.into())
    }

    /// Creates a new [`BrowserManager`].
    pub fn builder() -> BrowserManager {
        BrowserManager::new()
    }
}

impl From<Pool<BrowserManager>> for BrowserPool {
    fn from(pool: Pool<BrowserManager>) -> Self {
        Self { pool }
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
    fn call(&mut self, _req: ()) -> Self::Future {
        // self.pool.get().await
        todo!()
    }
}

#[cfg(test)]
mod test {
    use crate::backend::BrowserManager;
    use crate::context::Request;
    use crate::dataset::InMemDataset;
    use crate::{BoxError, Client, Result};

    #[test]
    pub fn builder() {
        let manager = BrowserManager::default();
        let _ = manager.build();
    }

    #[tokio::test]
    #[cfg(feature = "tracing")]
    #[tracing_test::traced_test]
    async fn noop() -> crate::Result<()> {
        use crate::backend::util::Trace;

        let pool = BrowserManager::default()
            .with_unmanaged("127.0.0.1:4444")
            .with_unmanaged("127.0.0.1:4445")
            .build();

        let backend = Trace::new(pool);
        let worker = Trace::default();

        let request = Request::get("https://example.com/").body(());
        let client = crate::Client::new(backend, worker)
            .with_request_queue(InMemDataset::stack())
            .with_dataset(InMemDataset::<u64>::new())
            .with_initial_request(request.unwrap());

        let _ = client.dataset::<u64>();
        let _ = client.run().await?;
        Ok(())
    }
}
