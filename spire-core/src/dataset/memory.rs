use std::collections::VecDeque;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use crate::dataset::Dataset;

/// A simple, thread-safe, in-memory dataset supporting both FIFO and LIFO ordering.
///
/// `InMemDataset` provides fast, lock-based storage for items that don't need
/// persistence across application restarts. It's ideal for URL queues, temporary
/// result buffers, or any ephemeral data during scraping operations.
///
/// # Ordering Modes
///
/// - **FIFO (First-In-First-Out)** - Created via [`queue()`](InMemDataset::queue),
///   processes items in the order they were added. Useful for breadth-first traversal.
/// - **LIFO (Last-In-First-Out)** - Created via [`stack()`](InMemDataset::stack),
///   processes most recently added items first. Useful for depth-first traversal.
///
/// # Characteristics
///
/// - **Thread-safe**: Uses `Arc<Mutex<_>>` internally for concurrent access
/// - **Cloneable**: Multiple handles can reference the same underlying storage
/// - **Infallible**: Operations never fail (error type is [`Infallible`])
/// - **Non-persistent**: Data is lost when the dataset is dropped
///
/// # Examples
///
/// ## FIFO Queue (Breadth-First)
///
/// ```ignore
/// use spire_core::dataset::{Dataset, InMemDataset};
///
/// # async fn example() -> Result<(), std::convert::Infallible> {
/// let queue = InMemDataset::<String>::queue();
///
/// queue.write("first".to_string()).await?;
/// queue.write("second".to_string()).await?;
///
/// assert_eq!(queue.read().await?, Some("first".to_string()));
/// assert_eq!(queue.read().await?, Some("second".to_string()));
/// # Ok(())
/// # }
/// ```
///
/// ## LIFO Stack (Depth-First)
///
/// ```ignore
/// use spire_core::dataset::{Dataset, InMemDataset};
///
/// # async fn example() -> Result<(), std::convert::Infallible> {
/// let stack = InMemDataset::<String>::stack();
///
/// stack.write("first".to_string()).await?;
/// stack.write("second".to_string()).await?;
///
/// assert_eq!(stack.read().await?, Some("second".to_string()));
/// assert_eq!(stack.read().await?, Some("first".to_string()));
/// # Ok(())
/// # }
/// ```
#[must_use]
pub struct InMemDataset<T> {
    inner: Arc<InMemDatasetInner<T>>,
}

struct InMemDatasetInner<T> {
    buffer: Mutex<VecDeque<T>>,
    is_fifo: bool,
}

impl<T> InMemDataset<T> {
    /// Creates an empty FIFO (queue) dataset.
    ///
    /// This is an alias for [`queue()`](InMemDataset::queue). Items will be
    /// processed in first-in-first-out order.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_core::dataset::InMemDataset;
    ///
    /// let dataset = InMemDataset::<String>::new();
    /// ```
    #[inline]
    pub fn new() -> Self {
        Self::queue()
    }

    /// Pre-allocates capacity for at least `additional` more elements.
    ///
    /// This can improve performance when you know approximately how many items
    /// will be stored, avoiding repeated allocations as the dataset grows.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_core::dataset::InMemDataset;
    ///
    /// // Pre-allocate space for 1000 URLs
    /// let dataset = InMemDataset::<String>::queue()
    ///     .reserved(1000);
    /// ```
    pub fn reserved(self, additional: usize) -> Self {
        {
            let mut guard = self
                .inner
                .buffer
                .lock()
                .expect("InMemDataset mutex poisoned");
            guard.reserve(additional);
        }

        self
    }

    /// Creates a FIFO (First-In-First-Out) dataset.
    ///
    /// Items are processed in the order they were added, making this suitable
    /// for breadth-first traversal patterns in web scraping.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_core::dataset::{Dataset, InMemDataset};
    ///
    /// # async fn example() -> Result<(), std::convert::Infallible> {
    /// let queue = InMemDataset::<&str>::queue();
    ///
    /// queue.write("first").await?;
    /// queue.write("second").await?;
    ///
    /// assert_eq!(queue.read().await?, Some("first"));  // FIFO order
    /// # Ok(())
    /// # }
    /// ```
    pub fn queue() -> Self {
        let inner = Arc::new(InMemDatasetInner {
            buffer: Mutex::new(VecDeque::new()),
            is_fifo: true,
        });

        Self { inner }
    }

    /// Creates a LIFO (Last-In-First-Out) dataset.
    ///
    /// Items are processed in reverse order (most recent first), making this
    /// suitable for depth-first traversal patterns in web scraping.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_core::dataset::{Dataset, InMemDataset};
    ///
    /// # async fn example() -> Result<(), std::convert::Infallible> {
    /// let stack = InMemDataset::<&str>::stack();
    ///
    /// stack.write("first").await?;
    /// stack.write("second").await?;
    ///
    /// assert_eq!(stack.read().await?, Some("second"));  // LIFO order
    /// # Ok(())
    /// # }
    /// ```
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
        self.inner
            .buffer
            .lock()
            .expect("InMemDataset mutex poisoned")
            .push_back(data);
        Ok(())
    }

    async fn read(&self) -> Result<Option<T>, Self::Error> {
        let mut guard = self
            .inner
            .buffer
            .lock()
            .expect("InMemDataset mutex poisoned");
        if self.inner.is_fifo {
            Ok(guard.pop_front())
        } else {
            Ok(guard.pop_back())
        }
    }

    fn len(&self) -> usize {
        let guard = self
            .inner
            .buffer
            .lock()
            .expect("InMemDataset mutex poisoned");
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
