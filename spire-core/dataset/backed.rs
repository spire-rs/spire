use std::sync::Arc;

use crate::dataset::{BoxDataset, Dataset, InMemDataset, Result};

/// Capacity limited InMemoryDataset backed by the BoxDataset
pub struct BackDataset<T> {
    inner: Arc<BackDatasetInner<T>>,
}

struct BackDatasetInner<T> {
    buffer: InMemDataset<T>,
    dataset: BoxDataset<T>,
}

impl<T> BackDataset<T> {
    pub fn new<D>(dataset: D) -> Self
    where
        D: Dataset<T>,
    {
        let inner = Arc::new(BackDatasetInner {
            buffer: InMemDataset::fifo(),
            dataset: BoxDataset::new(dataset),
        });

        Self { inner }
    }

    pub async fn flush() {}
}

impl<T> Drop for BackDataset<T> {
    fn drop(&mut self) {
        todo!()
    }
}

#[async_trait::async_trait]
impl<T> Dataset<T> for BackDataset<T>
where
    T: Send + Sync + 'static,
{
    async fn append(&self, data: T) -> Result<()> {
        todo!()
    }

    async fn evict(&self) -> Option<T> {
        todo!()
    }

    fn len(&self) -> usize {
        todo!()
    }
}
