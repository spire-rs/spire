use std::fmt;

use crate::dataset::Dataset;

/// Cloneable type-erased [`Dataset`] for a [`boxed_clone`] method.
///
/// [`boxed_clone`]: crate::dataset::DatasetExt::boxed_clone
pub struct BoxCloneDataset<T, E> {
    dataset: Box<dyn CloneDataset<T, Error = E>>,
}

trait CloneDataset<T>: Dataset<T> {
    fn clone_box(&self) -> Box<dyn CloneDataset<T, Error = Self::Error> + Send>;
}

impl<D, T> CloneDataset<T> for D
where
    D: Dataset<T> + Send + Clone + 'static, // TODO: Send? Clone?
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
        f.debug_struct("BoxCloneDataset").finish_non_exhaustive()
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
