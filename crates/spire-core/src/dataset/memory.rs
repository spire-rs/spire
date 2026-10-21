use std::collections::VecDeque;
use std::fmt;
use std::future::{Future, ready};
use std::sync::{Arc, Mutex};

use crate::Result;
use crate::dataset::Dataset;
use crate::sync::lock;

/// An in-memory [`Dataset`] read in insertion or reverse order.
///
/// Clones share the same storage.
pub struct InMemDataset<T> {
    inner: Arc<Inner<T>>,
}

struct Inner<T> {
    items: Mutex<VecDeque<T>>,
    lifo: bool,
}

impl<T> InMemDataset<T> {
    /// Creates a dataset read in insertion order.
    #[must_use]
    pub fn new() -> Self {
        Self::queue()
    }

    /// Creates a dataset read in insertion order.
    #[must_use]
    pub fn queue() -> Self {
        Self::with_order(false)
    }

    /// Creates a dataset that reads the most recent item first.
    #[must_use]
    pub fn stack() -> Self {
        Self::with_order(true)
    }

    fn with_order(lifo: bool) -> Self {
        let inner = Inner {
            items: Mutex::new(VecDeque::new()),
            lifo,
        };

        Self {
            inner: Arc::new(inner),
        }
    }
}

impl<T> Clone for InMemDataset<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Default for InMemDataset<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> FromIterator<T> for InMemDataset<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let dataset = Self::queue();
        lock(&dataset.inner.items).extend(iter);
        dataset
    }
}

impl<T> fmt::Debug for InMemDataset<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InMemDataset")
            .field("len", &lock(&self.inner.items).len())
            .field("lifo", &self.inner.lifo)
            .finish()
    }
}

impl<T: Send + 'static> Dataset<T> for InMemDataset<T> {
    fn write(&self, item: T) -> impl Future<Output = Result<()>> + Send {
        lock(&self.inner.items).push_back(item);
        ready(Ok(()))
    }

    fn read(&self) -> impl Future<Output = Result<Option<T>>> + Send {
        let mut items = lock(&self.inner.items);
        let item = if self.inner.lifo {
            items.pop_back()
        } else {
            items.pop_front()
        };
        ready(Ok(item))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(lock(&self.inner.items).len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn queue_and_stack_order() {
        let queue: InMemDataset<u32> = [1, 2].into_iter().collect();
        let stack = InMemDataset::stack();
        stack.write(1).await.unwrap();
        stack.write(2).await.unwrap();

        assert_eq!(queue.read().await.unwrap(), Some(1));
        assert_eq!(stack.read().await.unwrap(), Some(2));
        assert_eq!(queue.size_hint(), Some(1));
    }

    #[tokio::test]
    async fn clones_share_storage() {
        let dataset = InMemDataset::new();
        dataset.clone().write("item").await.unwrap();
        assert_eq!(dataset.read().await.unwrap(), Some("item"));
        assert_eq!(dataset.read().await.unwrap(), None);
    }
}
