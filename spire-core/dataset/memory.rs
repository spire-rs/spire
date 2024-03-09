use std::collections::VecDeque;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use crate::dataset::Dataset;

/// Simple in-memory [`Dataset`].
pub struct InMemDataset<T> {
    inner: Arc<InMemDatasetInner<T>>,
}

struct InMemDatasetInner<T> {
    buffer: Mutex<VecDeque<T>>,
    is_fifo: bool,
}

impl<T> InMemDataset<T> {
    /// Creates a new [`InMemDataset`].
    pub fn new() -> Self {
        Self::queue()
    }

    /// Creates a `First-In First-Out` [`InMemDataset`].
    /// Used for breadth-first traversal.
    pub fn queue() -> Self {
        let inner = Arc::new(InMemDatasetInner {
            buffer: Mutex::new(VecDeque::new()),
            is_fifo: true,
        });

        Self { inner }
    }

    /// Creates a `Last-In First-Out` [`InMemDataset`].
    /// Used for depth-first traversal.
    pub fn stack() -> Self {
        let inner = Arc::new(InMemDatasetInner {
            buffer: Mutex::new(VecDeque::new()),
            is_fifo: false,
        });

        Self { inner }
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
        Self::new()
    }
}

#[async_trait::async_trait]
impl<T> Dataset<T> for InMemDataset<T>
where
    T: Send + Sync + 'static,
{
    type Error = Infallible;

    async fn add(&self, data: T) -> Result<(), Self::Error> {
        let guard = self.inner.buffer.lock();
        let mut lock = guard.expect("should not be already held");
        lock.push_back(data);
        Ok(())
    }

    async fn get(&self) -> Result<Option<T>, Self::Error> {
        let guard = self.inner.buffer.lock();
        let mut lock = guard.expect("should not be already held");
        if self.inner.is_fifo {
            Ok(lock.pop_front())
        } else {
            Ok(lock.pop_back())
        }
    }

    fn len(&self) -> usize {
        let guard = self.inner.buffer.lock();
        let lock = guard.expect("should not be already held");
        lock.len()
    }
}
