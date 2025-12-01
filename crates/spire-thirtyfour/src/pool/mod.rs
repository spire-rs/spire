use std::task::{Context, Poll};

use deadpool::managed::Pool;
use futures::FutureExt;
use futures::future::BoxFuture;
use spire_core::{Error, Result};
use tower::Service;

/// Browser pool builder for configuring and creating pools.
pub mod builder;
/// Internal browser connection manager and lifecycle handling.
pub mod manager;

pub use builder::BrowserBuilder;
pub use manager::BrowserManager;

use crate::client::BrowserClient;
use crate::error::BrowserError;

/// WebDriver backend built on top of [`thirtyfour`] crate.
///
/// `BrowserPool` manages a pool of browser instances for web scraping tasks
/// that require JavaScript execution, dynamic content rendering, or user interaction
/// simulation.
///
/// Uses [`BrowserClient`] as the client implementation and supports multiple
/// WebDriver configurations with health monitoring and automatic cleanup.
///
/// # Examples
///
/// ```ignore
/// use spire_thirtyfour::BrowserPool;
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
        self.pool.get().await.map(Into::into).map_err(|_e| {
            BrowserError::pool_exhausted(0, 0).into() // TODO: Get actual pool metrics
        })
    }

    /// Creates a new [`BrowserBuilder`].
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_thirtyfour::BrowserPool;
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
    /// Creates a default browser pool with a single localhost connection.
    ///
    /// ## Note
    ///
    /// This creates a basic pool for development. For production use,
    /// use [`BrowserPool::builder`] instead with proper configuration.
    fn default() -> Self {
        BrowserPool::builder()
            .with_unmanaged("http://127.0.0.1:4444")
            .build()
            .expect("Failed to create default BrowserPool")
    }
}

impl Service<()> for BrowserPool {
    type Error = Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;
    type Response = BrowserClient;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Check if pool has available capacity
        // For now, we assume the pool is always ready (deadpool handles waiting)
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
    use spire_core::backend::utils::Noop;
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
            .with_unmanaged("http://127.0.0.1:4444")
            .with_unmanaged("http://127.0.0.1:4445")
            .build();

        let backend = pool?;
        let worker = Noop::default();

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
