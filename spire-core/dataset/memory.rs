use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::dataset::{Dataset, Result};

pub struct InMemDataset<T> {
    inner: Arc<InMemDatasetInner<T>>,
}

struct InMemDatasetInner<T> {
    buffer: Mutex<VecDeque<T>>,
}

impl<T> InMemDataset<T> {
    pub fn new() -> Self {
        todo!()
    }
}

impl<T> Clone for InMemDataset<T> {
    fn clone(&self) -> Self {
        todo!()
    }
}

impl<T> Default for InMemDataset<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Dataset<T> for InMemDataset<T> {
    fn append(&self, data: T) -> Result<()> {
        let mut guard = self.inner.buffer.lock();
        guard.expect("should not be already held").push_back(data);
        Ok(())
    }

    fn evict(&self) -> Option<T> {
        let mut guard = self.inner.buffer.lock();
        guard.expect("should not be already held").pop_front()
    }

    fn len(&self) -> usize {
        let mut guard = self.inner.buffer.lock();
        guard.expect("should not be already held").len()
    }
}
