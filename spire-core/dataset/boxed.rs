use std::fmt;
use std::sync::Arc;

use crate::dataset::{Dataset, Result};

/// Defines a boxed [`Dataset`] trait.
///
/// ....type-erased [`Dataset`].
pub struct BoxDataset<T> {
    inner: Arc<BoxDatasetInner<T>>,
}

struct BoxDatasetInner<T> {
    dataset: Box<dyn Dataset<T>>,
}

impl<T> BoxDataset<T> {
    pub fn new<D>(dataset: D) -> Self
    where
        D: Dataset<T> + 'static,
    {
        let inner = Arc::new(BoxDatasetInner {
            dataset: Box::new(dataset),
        });

        Self { inner }
    }
}

impl<T> fmt::Debug for BoxDataset<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxDataset").finish_non_exhaustive()
    }
}

impl<T> Clone for BoxDataset<T> {
    fn clone(&self) -> Self {
        let inner = self.inner.clone();
        Self { inner }
    }
}

#[async_trait::async_trait]
impl<T> Dataset<T> for BoxDataset<T>
where
    T: Send + Sync + 'static,
{
    async fn append(&self, data: T) -> Result<()> {
        self.inner.dataset.append(data).await
    }

    async fn evict(&self) -> Option<T> {
        self.inner.dataset.evict().await
    }

    fn len(&self) -> usize {
        self.inner.dataset.len()
    }
}
