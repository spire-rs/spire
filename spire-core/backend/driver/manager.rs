use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex};

use deadpool::managed::{Manager, Metrics, Pool, RecycleResult};
use fantoccini::Client as WebClient;

use crate::backend::BrowserPool;
use crate::Error;

#[derive(Debug, Clone)]
pub struct BrowserConn {
    pub id: String,
    pub client: WebClient,
}

/// [`BrowserPool`] builder. Manages browser connection and/or process.
#[derive(Clone)]
pub struct BrowserManager {
    inner: Arc<BrowserManagerInner>,
}

struct BrowserManagerInner {
    // builder: ClientBuilder<()>,
    unmanaged_free: Mutex<Vec<String>>,
    unmanaged_conn: Mutex<VecDeque<BrowserConn>>,
}

impl BrowserManager {
    /// Creates a new [`BrowserManager`].
    pub fn new() -> Self {
        // BLOCKED: https://github.com/jonhoo/fantoccini/pull/245
        todo!()
    }

    /// TODO.
    pub fn with_unmanaged(mut self, webdriver: impl AsRef<str>) -> Self {
        {
            let mut guard = self.inner.unmanaged_free.lock().unwrap();
            guard.push(webdriver.as_ref().to_owned());
        }

        self
    }

    /// TODO.
    pub fn with_managed<F, Fut>(self, f: F) -> Self {
        todo!()
    }

    /// Constructs a new [`BrowserPool`].
    pub fn build(self) -> BrowserPool {
        let pool = Pool::builder(self).build();
        BrowserPool::new(pool.expect("should not require runtime"))
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

impl Manager for BrowserManager {
    type Type = BrowserConn;
    type Error = Error;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        let mut guard = self.inner.unmanaged_conn.lock().unwrap();
        let conn = guard.pop_front().unwrap();

        Ok(conn)
    }

    async fn recycle(
        &self,
        client: &mut Self::Type,
        metrics: &Metrics,
    ) -> RecycleResult<Self::Error> {
        // TODO: Metrics.
        // let _ = metrics.recycled.unwrap_or_else(Instant::now);
        //
        // let inner = client.clone().into_inner();
        // inner.close().await.map_err(Error::new)?;
        // self.release(client.id());

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::backend::driver::BrowserManager;

    #[test]
    fn with_unmanaged() {
        let _ = BrowserManager::default()
            .with_unmanaged("127.0.0.1:4444")
            .with_unmanaged("127.0.0.1:4445")
            .build();
    }

    #[test]
    fn with_managed() {
        // TODO.
        let _ = BrowserManager::default().build();
    }
}
