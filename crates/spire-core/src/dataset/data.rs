use std::fmt;
use std::future::Future;
use std::sync::Arc;

use futures::future::BoxFuture;
use futures::{FutureExt, Stream, stream};

use crate::Result;
use crate::dataset::Dataset;

/// How many items [`Data::read_all`] takes at once.
pub const READ_BATCH: usize = 1024;

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

    /// Stores every item of `items` at once, see [`Dataset::write_batch`].
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying dataset fails.
    pub async fn write_all(&self, items: impl IntoIterator<Item = T>) -> Result<()> {
        let items: Vec<T> = items.into_iter().collect();
        if items.is_empty() {
            return Ok(());
        }
        self.inner.write_batch(items).await
    }

    /// Takes the next item, or `None` if the dataset is empty.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying dataset fails.
    pub async fn read(&self) -> Result<Option<T>> {
        self.inner.read().await
    }

    /// Takes up to `max` items at once, see [`Dataset::read_batch`].
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying dataset fails.
    pub async fn read_batch(&self, max: usize) -> Result<Vec<T>> {
        self.inner.read_batch(max).await
    }

    /// Takes every item until the dataset is empty, in batches of
    /// [`READ_BATCH`] items.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying dataset fails.
    pub async fn read_all(&self) -> Result<Vec<T>> {
        let mut items = Vec::with_capacity(self.size_hint().unwrap_or_default());
        loop {
            let batch = self.inner.read_batch(READ_BATCH).await?;
            let last = batch.len() < READ_BATCH;
            items.extend(batch);
            if last {
                return Ok(items);
            }
        }
    }

    /// Returns a stream that takes items one at a time until the dataset is
    /// empty, so it never takes an item it does not yield. To drain a large
    /// dataset, [`into_batches`](Self::into_batches) takes fewer trips to
    /// its storage.
    pub fn into_stream(self) -> impl Stream<Item = Result<T>> + Send {
        stream::try_unfold(self, |data| async move {
            let item = data.read().await?;
            Ok(item.map(|item| (item, data)))
        })
    }

    /// Returns a stream that takes batches of up to `max` items, see
    /// [`Dataset::read_batch`], until the dataset is empty.
    pub fn into_batches(self, max: usize) -> impl Stream<Item = Result<Vec<T>>> + Send {
        let max = max.max(1);
        stream::try_unfold((self, false), move |(data, drained)| async move {
            if drained {
                return Ok(None);
            }
            let batch = data.read_batch(max).await?;
            if batch.is_empty() {
                return Ok(None);
            }
            let drained = batch.len() < max;
            Ok(Some((batch, (data, drained))))
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

    fn write_batch(&self, items: Vec<T>) -> impl Future<Output = Result<()>> + Send {
        self.inner.write_batch(items)
    }

    fn read(&self) -> impl Future<Output = Result<Option<T>>> + Send {
        self.inner.read()
    }

    fn read_batch(&self, max: usize) -> impl Future<Output = Result<Vec<T>>> + Send {
        self.inner.read_batch(max)
    }

    fn size_hint(&self) -> Option<usize> {
        self.inner.size_hint()
    }
}

/// An object-safe [`Dataset`].
trait DynDataset<T>: Send + Sync + 'static {
    fn write(&self, item: T) -> BoxFuture<'_, Result<()>>;
    fn write_batch(&self, items: Vec<T>) -> BoxFuture<'_, Result<()>>;
    fn read(&self) -> BoxFuture<'_, Result<Option<T>>>;
    fn read_batch(&self, max: usize) -> BoxFuture<'_, Result<Vec<T>>>;
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

    fn write_batch(&self, items: Vec<T>) -> BoxFuture<'_, Result<()>> {
        Dataset::write_batch(self, items).boxed()
    }

    fn read(&self) -> BoxFuture<'_, Result<Option<T>>> {
        Dataset::read(self).boxed()
    }

    fn read_batch(&self, max: usize) -> BoxFuture<'_, Result<Vec<T>>> {
        Dataset::read_batch(self, max).boxed()
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

    #[tokio::test]
    async fn batches_stream_until_the_dataset_is_empty() {
        let data = Data::new((1..=5).collect::<InMemDataset<u32>>());
        let batches: Vec<Vec<u32>> = data.clone().into_batches(2).try_collect().await.unwrap();
        assert_eq!(batches, [vec![1, 2], vec![3, 4], vec![5]]);

        // A last full batch ends on the next, empty read.
        data.write_all([6, 7]).await.unwrap();
        let batches: Vec<Vec<u32>> = data.clone().into_batches(2).try_collect().await.unwrap();
        assert_eq!(batches, [vec![6, 7]]);
        let batches: Vec<Vec<u32>> = data.into_batches(2).try_collect().await.unwrap();
        assert_eq!(batches, Vec::<Vec<u32>>::new());
    }
}
