use std::fmt;

use crate::dataset::Dataset;

/// Type-erased, boxed [`Dataset`].
pub struct BoxDataset<T, E> {
    dataset: Box<dyn Dataset<T, Error = E>>,
}

impl<T, E> BoxDataset<T, E> {
    /// Creates a new [`BoxDataset`].
    pub fn new<D>(dataset: D) -> Self
    where
        D: Dataset<T, Error = E> + 'static,
    {
        let dataset = Box::new(dataset);
        Self { dataset }
    }
}

impl<T, E> fmt::Debug for BoxDataset<T, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxDataset").finish()
    }
}

#[async_trait::async_trait]
impl<T, E> Dataset<T> for BoxDataset<T, E>
where
    T: Send + Sync + 'static,
    E: 'static,
{
    type Error = E;

    async fn add(&self, data: T) -> Result<(), Self::Error> {
        self.dataset.add(data).await
    }

    async fn get(&self) -> Result<Option<T>, Self::Error> {
        self.dataset.get().await
    }

    fn len(&self) -> usize {
        self.dataset.len()
    }
}
