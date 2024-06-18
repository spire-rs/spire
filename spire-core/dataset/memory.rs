use std::collections::VecDeque;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use crate::dataset::Dataset;

/// Simple in-memory `FIFO` or `LIFO` [`Dataset`].
#[must_use]
pub struct InMemDataset<T> {
    inner: Arc<InMemDatasetInner<T>>,
}

struct InMemDatasetInner<T> {
    buffer: Mutex<VecDeque<T>>,
    is_fifo: bool,
}

impl<T> InMemDataset<T> {
    /// Creates an empty [`InMemDataset`].
    ///
    /// Same as [`InMemDataset::queue`].
    #[inline]
    pub fn new() -> Self {
        Self::queue()
    }

    /// Reserves capacity for at least `additional` more elements to be inserted.
    pub fn reserved(self, additional: usize) -> Self {
        {
            let mut guard = self.inner.buffer.lock().unwrap();
            guard.reserve(additional);
        }

        self
    }

    /// Creates a `First-In First-Out` [`InMemDataset`].
    ///
    /// Used for breadth-first traversal.
    pub fn queue() -> Self {
        let inner = Arc::new(InMemDatasetInner {
            buffer: Mutex::new(VecDeque::new()),
            is_fifo: true,
        });

        Self { inner }
    }

    /// Creates a `Last-In First-Out` [`InMemDataset`].
    ///
    /// Used for depth-first traversal.
    pub fn stack() -> Self {
        let inner = Arc::new(InMemDatasetInner {
            buffer: Mutex::new(VecDeque::new()),
            is_fifo: false,
        });

        Self { inner }
    }
}

impl<T> Clone for InMemDataset<T> {
    fn clone(&self) -> Self {
        let inner = self.inner.clone();
        Self { inner }
    }
}

impl<T> Default for InMemDataset<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<V, T> From<V> for InMemDataset<T>
where
    V: IntoIterator<Item = T>,
{
    fn from(value: V) -> Self {
        let vec: VecDeque<_> = value.into_iter().collect();
        let inner = Arc::new(InMemDatasetInner {
            buffer: Mutex::new(vec),
            is_fifo: true,
        });

        Self { inner }
    }
}

#[async_trait::async_trait]
impl<T> Dataset<T> for InMemDataset<T>
where
    T: Send + Sync + 'static,
{
    type Error = Infallible;

    async fn write(&self, data: T) -> Result<(), Self::Error> {
        self.inner.buffer.lock().unwrap().push_back(data);
        Ok(())
    }

    async fn read(&self) -> Result<Option<T>, Self::Error> {
        let mut guard = self.inner.buffer.lock().unwrap();
        if self.inner.is_fifo {
            Ok(guard.pop_front())
        } else {
            Ok(guard.pop_back())
        }
    }

    fn len(&self) -> usize {
        let guard = self.inner.buffer.lock().unwrap();
        guard.len()
    }
}

#[cfg(test)]
mod test {
    use crate::dataset::{Dataset, InMemDataset};
    use crate::Result;

    #[tokio::test]
    async fn queue() -> Result<()> {
        let dataset = InMemDataset::queue();

        dataset.write(1).await?;
        dataset.write(2).await?;

        let data = dataset.read().await?;
        assert_eq!(data, Some(1));

        Ok(())
    }

    #[tokio::test]
    async fn stack() -> Result<()> {
        let dataset = InMemDataset::stack();

        dataset.write(1).await?;
        dataset.write(2).await?;

        let data = dataset.read().await?;
        assert_eq!(data, Some(2));

        Ok(())
    }
}
