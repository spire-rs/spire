use std::task::{Context, Poll};

use deadpool::managed::Pool;
use futures::future::BoxFuture;
use futures::FutureExt;
use tower::Service;

use spire_core::{Error, ErrorKind, Result};

use crate::builder::BrowserBuilder;
use crate::client::BrowserClient;
use crate::manager::BrowserManager;

/// WebDriver backend built on top of [`fantoccini`] crate.
///
/// `BrowserPool` manages a pool of browser instances for web scraping tasks
/// that require JavaScript execution, dynamic content rendering, or user interaction
/// simulation.
///
/// Uses [`BrowserClient`] as the client implementation.
///
/// # Examples
///
/// ```ignore
/// use spire_fantoccini::BrowserPool;
/// use spire_core::Client;
///
/// let pool = BrowserPool::builder()
///     .with_unmanaged("127.0.0.1:4444")
///     .with_unmanaged("127.0.0.1:4445")
///     .build();
///
/// let client = Client::new(pool, worker);
/// client.run().await?;
/// ```
///
/// [`Backend`]: spire_core::backend::Backend
/// [`Client`]: spire_core::backend::Client
#[must_use = "services do nothing unless you `.poll_ready` or `.call` them"]
#[derive(Clone)]
pub struct BrowserPool {
    pool: Pool<BrowserManager>,
}

impl BrowserPool {
    /// Creates a new [`BrowserPool`].
    #[inline]
    pub(crate) fn new(pool: Pool<BrowserManager>) -> Self {
        Self { pool }
    }

    /// Waits and returns an available [`BrowserClient`].
    async fn client(&self) -> Result<BrowserClient> {
        self.pool.get().await.map(Into::into).map_err(|e| {
            Error::new(
                ErrorKind::Backend,
                format!("Failed to get browser from pool: {}", e),
            )
        })
    }

    /// Creates a new [`BrowserBuilder`].
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
    pub fn builder() -> BrowserBuilder {
        BrowserBuilder::default()
    }
}

impl From<Pool<BrowserManager>> for BrowserPool {
    #[inline]
    fn from(pool: Pool<BrowserManager>) -> Self {
        Self::new(pool)
    }
}

impl Default for BrowserPool {
    /// Creates a default browser pool.
    ///
    /// ## Note
    ///
    /// This implementation is currently not available and will panic.
    /// Use [`BrowserPool::builder`] instead.
    fn default() -> Self {
        todo!("impl Default for BrowserPool - use BrowserPool::builder() instead")
    }
}

impl Service<()> for BrowserPool {
    type Response = BrowserClient;
    type Error = Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // TODO: Check for available browsers in the pool
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, _req: ()) -> Self::Future {
        let this = self.clone();
        let fut = async move { this.client().await };

        fut.boxed()
    }
}

#[cfg(test)]
mod test {
    use spire_core::backend::utils::{Noop, Trace};
    use spire_core::context::Request;
    use spire_core::dataset::InMemDataset;
    use spire_core::{Client, Result};

    use crate::BrowserPool;

    #[test]
    fn build() {
        let _ = BrowserPool::builder().build();
    }

    #[tokio::test]
    async fn noop() -> Result<()> {
        let pool = BrowserPool::builder()
            .with_unmanaged("127.0.0.1:4444")
            .with_unmanaged("127.0.0.1:4445")
            .build();

        let backend = Trace::new(pool);
        let worker = Trace::new(Noop::default());

        let request = Request::get("https://example.com/").body(());
        let client = Client::new(backend, worker)
            .with_request_queue(InMemDataset::stack())
            .with_dataset(InMemDataset::<u64>::new())
            .with_initial_request(request.unwrap());

        let _ = client.dataset::<u64>();
        let _ = client.run().await?;

        Ok(())
    }
}
