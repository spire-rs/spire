use std::fmt;
use std::future::Future;
use std::sync::Arc;

use futures::future::BoxFuture;
use futures::{FutureExt, Stream, stream};

use crate::Result;
use crate::dataset::Dataset;

/// A cloneable, type-erased handle to a [`Dataset`].
pub struct Data<T> {
    inner: Arc<dyn DynDataset<T>>,
}

impl<T: Send + 'static> Data<T> {
    /// Wraps `dataset`.
    #[must_use]
    pub fn new(dataset: impl Dataset<T>) -> Self {
        Self {
            inner: Arc::new(dataset),
        }
    }

    /// Stores an item.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying dataset fails.
    pub async fn write(&self, item: T) -> Result<()> {
        self.inner.write(item).await
    }

    /// Stores every item of `items`, stopping at the first error.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying dataset fails.
    pub async fn write_all(&self, items: impl IntoIterator<Item = T>) -> Result<()> {
        for item in items {
            self.inner.write(item).await?;
        }
        Ok(())
    }

    /// Takes the next item, or `None` if the dataset is empty.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying dataset fails.
    pub async fn read(&self) -> Result<Option<T>> {
        self.inner.read().await
    }

    /// Takes every item until the dataset is empty.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying dataset fails.
    pub async fn read_all(&self) -> Result<Vec<T>> {
        let mut items = Vec::with_capacity(self.size_hint().unwrap_or_default());
        while let Some(item) = self.inner.read().await? {
            items.push(item);
        }
        Ok(items)
    }

    /// Returns a stream that takes items until the dataset is empty.
    pub fn into_stream(self) -> impl Stream<Item = Result<T>> + Send {
        stream::try_unfold(self, |data| async move {
            let item = data.read().await?;
            Ok(item.map(|item| (item, data)))
        })
    }

    /// Returns the number of stored items, if known.
    #[must_use]
    pub fn size_hint(&self) -> Option<usize> {
        self.inner.size_hint()
    }
}

impl<T> Clone for Data<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> fmt::Debug for Data<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Data")
            .field("type", &std::any::type_name::<T>())
            .finish_non_exhaustive()
    }
}

impl<T: Send + 'static> Dataset<T> for Data<T> {
    fn write(&self, item: T) -> impl Future<Output = Result<()>> + Send {
        self.inner.write(item)
    }

    fn read(&self) -> impl Future<Output = Result<Option<T>>> + Send {
        self.inner.read()
    }

    fn size_hint(&self) -> Option<usize> {
        self.inner.size_hint()
    }
}

/// An object-safe [`Dataset`].
trait DynDataset<T>: Send + Sync + 'static {
    fn write(&self, item: T) -> BoxFuture<'_, Result<()>>;
    fn read(&self) -> BoxFuture<'_, Result<Option<T>>>;
    fn size_hint(&self) -> Option<usize>;
}

impl<T, D> DynDataset<T> for D
where
    T: Send + 'static,
    D: Dataset<T>,
{
    fn write(&self, item: T) -> BoxFuture<'_, Result<()>> {
        Dataset::write(self, item).boxed()
    }

    fn read(&self) -> BoxFuture<'_, Result<Option<T>>> {
        Dataset::read(self).boxed()
    }

    fn size_hint(&self) -> Option<usize> {
        Dataset::size_hint(self)
    }
}

#[cfg(test)]
mod tests {
    use futures::TryStreamExt;

    use super::*;
    use crate::dataset::InMemDataset;

    #[tokio::test]
    async fn batch_and_stream() {
        let data = Data::new(InMemDataset::new());
        data.write_all([1, 2, 3]).await.unwrap();
        assert_eq!(data.size_hint(), Some(3));
        assert_eq!(data.read().await.unwrap(), Some(1));
        assert_eq!(
            data.clone()
                .into_stream()
                .try_collect::<Vec<_>>()
                .await
                .unwrap(),
            [2, 3]
        );
        assert_eq!(data.read_all().await.unwrap(), Vec::<i32>::new());
    }
}
