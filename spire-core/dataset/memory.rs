use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::dataset::{Dataset, Result};

/// Simple in-memory [`Dataset`].
pub struct InMemDataset<T> {
    inner: Arc<InMemDatasetInner<T>>,
}

struct InMemDatasetInner<T> {
    buffer: Mutex<VecDeque<T>>,
    fifo: AtomicBool,
}

impl<T> InMemDataset<T> {
    /// Creates a `First-In First-Out` [`InMemDataset`].
    pub fn fifo() -> Self {
        let inner = Arc::new(InMemDatasetInner {
            buffer: Mutex::new(VecDeque::new()),
            fifo: AtomicBool::new(true),
        });

        Self { inner }
    }

    /// Creates a `Last-In First-Out` [`InMemDataset`].
    pub fn lifo() -> Self {
        let inner = Arc::new(InMemDatasetInner {
            buffer: Mutex::new(VecDeque::new()),
            fifo: AtomicBool::new(false),
        });

        Self { inner }
    }

    /// Changes `FIFO` to `LIFO` modes, or vice versa.
    pub fn flip(self) -> Self {
        let _ = self.inner.fifo.fetch_xor(true, Ordering::SeqCst);
        self
    }
}

impl<T> Clone for InMemDataset<T> {
    fn clone(&self) -> Self {
        let inner = self.inner.clone();
        Self { inner }
    }
}

impl<T> Default for InMemDataset<T> {
    fn default() -> Self {
        Self::fifo()
    }
}

#[async_trait::async_trait]
impl<T> Dataset<T> for InMemDataset<T>
where
    T: Send + Sync + 'static,
{
    async fn append(&self, data: T) -> Result<()> {
        let guard = self.inner.buffer.lock();
        let mut lock = guard.expect("should not be already held");
        lock.push_back(data);
        Ok(())
    }

    async fn evict(&self) -> Option<T> {
        let guard = self.inner.buffer.lock();
        let mut lock = guard.expect("should not be already held");
        if self.inner.fifo.load(Ordering::SeqCst) {
            lock.pop_front()
        } else {
            lock.pop_back()
        }
    }

    fn len(&self) -> usize {
        let guard = self.inner.buffer.lock();
        let lock = guard.expect("should not be already held");
        lock.len()
    }
}
