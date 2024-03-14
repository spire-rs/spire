use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::time::Instant;

use deadpool::managed::{Manager, Metrics, Pool, RecycleResult};

use crate::backend::{BrowserClient, BrowserPool};
use crate::Error;

/// [`BrowserPool`] builder.
pub struct BrowserManager {
    managed: HashMap<u32, ()>,
    // builder: ClientBuilder<()>,
    // webdriver: Vec<()>
}

impl BrowserManager {
    pub fn new() -> Self {
        Self {
            managed: HashMap::default(),
        }
    }

    pub fn with_unmanaged<T>(self, webdriver: T) -> Self
    where
        T: AsRef<str>,
    {
        let webdriver = webdriver.as_ref().to_owned();
        todo!()
    }

    pub fn with_managed<F, Fut>(self, f: F) -> Self
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Option<()>>,
    {
        todo!()
    }

    pub fn build(self) -> BrowserPool {
        let pool = Pool::builder(self).build();
        BrowserPool::new(pool.expect("should not require runtime"))
    }

    fn capture(&self) -> u32 {
        todo!()
    }

    fn release(&self, id: u32) {
        todo!()
    }
}

impl Default for BrowserManager {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for BrowserManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserManager").finish_non_exhaustive()
    }
}

#[deadpool::async_trait]
impl Manager for BrowserManager {
    type Type = BrowserClient;
    type Error = Error;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        let _ = self.capture();
        todo!()
    }

    async fn recycle(
        &self,
        client: &mut Self::Type,
        metrics: &Metrics,
    ) -> RecycleResult<Self::Error> {
        // TODO: Metrics.
        let _ = metrics.recycled.unwrap_or_else(Instant::now);

        let inner = client.clone().into_inner();
        inner.close().await.map_err(Error::new)?;
        self.release(client.id());

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::backend::driver::BrowserManager;

    #[test]
    fn build() {
        let _ = BrowserManager::default()
            .with_unmanaged("127.0.0.1:4444")
            .with_unmanaged("127.0.0.1:4445")
            .build();
    }
}
