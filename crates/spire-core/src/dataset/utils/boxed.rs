use std::fmt;

use crate::dataset::Dataset;

/// Type-erased wrapper for [`Dataset`] implementations.
///
/// `BoxDataset` allows storing different concrete dataset types behind a trait
/// object, enabling runtime polymorphism. This is useful when you need to store
/// datasets of the same data and error types but different implementations.
///
/// Unlike [`BoxCloneDataset`], this type cannot be cloned.
///
/// # Examples
///
/// ```ignore
/// use spire_core::dataset::{Dataset, DatasetExt, InMemDataset};
///
/// # async fn example() -> Result<(), std::convert::Infallible> {
/// // Store different dataset implementations in a collection
/// let datasets: Vec<BoxDataset<String, std::convert::Infallible>> = vec![
///     InMemDataset::queue().boxed(),
///     InMemDataset::stack().boxed(),
/// ];
///
/// for dataset in datasets {
///     dataset.write("data".to_string()).await?;
/// }
/// # Ok(())
/// # }
/// ```
///
/// # Type Erasure
///
/// Type erasure allows you to work with datasets generically without knowing
/// their concrete types at compile time:
///
/// ```ignore
/// use spire_core::dataset::{BoxDataset, DatasetExt, InMemDataset};
///
/// fn create_dataset(use_queue: bool) -> BoxDataset<i32, std::convert::Infallible> {
///     if use_queue {
///         InMemDataset::queue().boxed()
///     } else {
///         InMemDataset::stack().boxed()
///     }
/// }
/// ```
///
/// [`boxed`]: crate::dataset::DatasetExt::boxed
/// [`BoxCloneDataset`]: crate::dataset::BoxCloneDataset
#[must_use]
pub struct BoxDataset<T, E> {
    dataset: Box<dyn Dataset<T, Error = E>>,
}

impl<T, E> BoxDataset<T, E> {
    /// Creates a new [`BoxDataset`].
    #[inline]
    pub fn new<D>(dataset: D) -> Self
    where
        D: Dataset<T, Error = E> + 'static,
    {
        let dataset = Box::new(dataset);
        Self { dataset }
    }
}

impl<T, E> fmt::Debug for BoxDataset<T, E> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxDataset").finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl<T, E> Dataset<T> for BoxDataset<T, E>
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
