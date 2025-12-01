use std::fmt;

use crate::dataset::Dataset;

/// Cloneable type-erased wrapper for [`Dataset`] implementations.
///
/// `BoxCloneDataset` provides type erasure like [`BoxDataset`], but with the
/// additional ability to clone the wrapper. This is particularly useful when
/// you need to share datasets across multiple consumers or store them in
/// cloneable contexts.
///
/// # Examples
///
/// ```ignore
/// use spire_core::dataset::{Dataset, DatasetExt, InMemDataset};
///
/// # async fn example() -> Result<(), std::convert::Infallible> {
/// let dataset = InMemDataset::<String>::queue().boxed_clone();
///
/// // Clone the dataset to share it
/// let dataset_clone = dataset.clone();
///
/// dataset.write("hello".to_string()).await?;
/// assert_eq!(dataset_clone.read().await?, Some("hello".to_string()));
/// # Ok(())
/// # }
/// ```
///
/// # Sharing Across Threads
///
/// `BoxCloneDataset` can be cloned and sent across thread boundaries:
///
/// ```ignore
/// use spire_core::dataset::{Dataset, DatasetExt, InMemDataset};
///
/// # async fn example() -> Result<(), std::convert::Infallible> {
/// let dataset = InMemDataset::<i32>::queue().boxed_clone();
///
/// let dataset_clone = dataset.clone();
/// tokio::spawn(async move {
///     dataset_clone.write(42).await.unwrap();
/// });
///
/// // Original dataset shares the same underlying storage
/// tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
/// assert_eq!(dataset.read().await?, Some(42));
/// # Ok(())
/// # }
/// ```
///
/// [`boxed_clone`]: crate::dataset::DatasetExt::boxed_clone
/// [`BoxDataset`]: crate::dataset::utils::BoxDataset
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
