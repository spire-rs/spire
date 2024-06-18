use std::fmt;

use crate::dataset::Dataset;

/// Cloneable type-erased [`Dataset`] for a [`boxed_clone`] method.
///
/// [`boxed_clone`]: crate::dataset::DatasetExt::boxed_clone
#[must_use]
pub struct BoxCloneDataset<T, E> {
    dataset: Box<dyn CloneBoxDataset<T, Error = E>>,
}

trait CloneBoxDataset<T>: Dataset<T> + 'static {
    fn clone_box(&self) -> Box<dyn CloneBoxDataset<T, Error = Self::Error> + Send>;
}

impl<D, T> CloneBoxDataset<T> for D
where
    D: Dataset<T> + Clone + 'static,
{
    #[inline]
    fn clone_box(&self) -> Box<dyn CloneBoxDataset<T, Error = D::Error> + Send> {
        Box::new(self.clone())
    }
}

impl<T, E> BoxCloneDataset<T, E> {
    /// Creates a new [`BoxCloneDataset`].
    #[inline]
    pub fn new<D>(dataset: D) -> Self
    where
        D: Dataset<T, Error = E> + Clone + 'static,
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
    #[inline]
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

    #[inline]
    async fn write(&self, data: T) -> Result<(), Self::Error> {
        self.dataset.write(data).await
    }

    #[inline]
    async fn read(&self) -> Result<Option<T>, Self::Error> {
        self.dataset.read().await
    }

    #[inline]
    fn len(&self) -> usize {
        self.dataset.len()
    }
}
