use std::task::{Context, Poll};

use deadpool::managed::Pool;
use futures::future::BoxFuture;
use futures::FutureExt;
use tower::Service;

pub use builder::BrowserBuilder;
pub use client::BrowserClient;
use connect::BrowserConnection;
use manager::BrowserManager;

use crate::{Error, Result};

mod builder;
mod client;
mod connect;
mod manager;

/// Web-driver [`Backend`] built on top of [`fantoccini`] crate.
/// Uses [`BrowserClient`] as a [`Client`].
///
/// [`Backend`]: crate::backend::Backend
/// [`Client`]: crate::backend::Client
#[must_use = "services do nothing unless you `.poll_ready` or `.call` them"]
#[derive(Clone)]
pub struct BrowserPool {
    pool: Pool<BrowserManager>,
}

impl BrowserPool {
    /// Creates a new [`BrowserPool`].
    #[inline]
    pub fn new(pool: Pool<BrowserManager>) -> Self {
        Self { pool }
    }

    /// Waits and returns an available [`BrowserClient`].
    async fn client(&self) -> Result<BrowserClient> {
        self.pool.get().await.map(Into::into).map_err(Into::into)
    }

    /// Creates a new [`BrowserBuilder`].
    #[inline]
    fn builder() -> BrowserBuilder {
        BrowserBuilder::default()
    }
}

impl From<Pool<BrowserManager>> for BrowserPool {
    #[inline]
    fn from(pool: Pool<BrowserManager>) -> Self {
        Self::new(pool)
    }
}

impl Service<()> for BrowserPool {
    type Response = BrowserClient;
    type Error = Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // TODO: Check for available browsers.
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, _req: ()) -> Self::Future {
        let this = self.clone();
        let fut = async move { this.client().await };

        fut.boxed()
    }
}

#[test]
mod test {
    use crate::backend::BrowserPool;

    #[test]
    fn build() {
        let _ = BrowserPool::builder().build();
    }

    #[tokio::test]
    #[cfg(feature = "tracing")]
    #[tracing_test::traced_test]
    async fn noop() -> crate::Result<()> {
        use crate::backend::util::Trace;

        let pool = BrowserPool::builder()
            .with_unmanaged("127.0.0.1:4444")
            .with_unmanaged("127.0.0.1:4445")
            .build();

        let backend = Trace::new(pool);
        let worker = Trace::new(Noop::default());

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
