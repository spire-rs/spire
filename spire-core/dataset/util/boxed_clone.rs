use std::fmt;

use async_stream::try_stream;
use futures::stream::Stream;

use crate::dataset::Dataset;

/// Cloneable type-erased [`Dataset`].
pub struct BoxCloneDataset<T, E> {
    dataset: Box<dyn CloneDataset<T, Error = E>>,
}

trait CloneDataset<T>: Dataset<T> {
    fn clone_box(&self) -> Box<dyn CloneDataset<T, Error = Self::Error> + Send>;
}

impl<D, T> CloneDataset<T> for D
where
    D: Dataset<T> + Send + Clone + 'static,
{
    fn clone_box(&self) -> Box<dyn CloneDataset<T, Error = D::Error> + Send> {
        Box::new(self.clone())
    }
}

impl<T, E> BoxCloneDataset<T, E> {
    /// Creates a new [`BoxCloneDataset`].
    pub fn new<D>(dataset: D) -> Self
    where
        D: Dataset<T, Error = E> + Clone,
    {
        let dataset = Box::new(dataset);
        Self { dataset }
    }

    // TODO: Expose into_stream.
    pub(crate) fn into_stream(self) -> impl Stream<Item = Result<T, E>>
    where
        T: 'static,
        E: 'static,
    {
        try_stream! {
            loop {
                let item = self.dataset.get().await?;
                if let Some(item) = item {
                    yield item;
                }
            }
        }
    }
}

impl<T, E> Clone for BoxCloneDataset<T, E>
where
    T: 'static,
    E: 'static,
{
    fn clone(&self) -> Self {
        let dataset = self.dataset.clone_box();
        Self { dataset }
    }
}

impl<T, E> fmt::Debug for BoxCloneDataset<T, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxCloneDataset").finish()
    }
}

#[async_trait::async_trait]
impl<T, E> Dataset<T> for BoxCloneDataset<T, E>
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
