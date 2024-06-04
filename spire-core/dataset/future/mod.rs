//! Implementation of [`futures`] traits.
//!

use std::fmt;

pub use sink::DataSink;
pub use stream::DataStream;

use crate::dataset::util::BoxCloneDataset;
use crate::dataset::Dataset;
use crate::Error;

mod sink;
mod stream;

// TODO: Stream + Sink.

/// Convenient [`BoxCloneDataset`] wrapper.
#[must_use]
#[derive(Clone)]
pub struct Data<T, E = Error>(pub BoxCloneDataset<T, E>)
where
    T: 'static,
    E: 'static;

impl<T, E> Data<T, E>
where
    T: Send + Sync + 'static,
    E: Send + Sync + 'static,
{
    /// Creates a new [`Data`].
    #[inline]
    pub const fn new(inner: BoxCloneDataset<T, E>) -> Self {
        Self(inner)
    }

    /// Returns the underlying [`BoxCloneDataset`].
    #[inline]
    pub fn into_inner(self) -> BoxCloneDataset<T, E> {
        self.0
    }

    /// Returns a new [`DataStream`].
    #[inline]
    pub fn into_stream(self) -> DataStream<T, E> {
        self.into_inner().into_stream()
    }

    /// Returns a new [`DataSink`].
    #[inline]
    pub fn into_sink(self) -> DataSink<T, E> {
        self.into_inner().into_sink()
    }
}

impl<T> fmt::Debug for Data<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Dataset").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod test {
    use futures::{SinkExt, StreamExt};

    use crate::dataset::{Dataset, InMemDataset};
    use crate::Result;

    #[tokio::test]
    async fn memory() -> Result<()> {
        let dataset = InMemDataset::<i32>::new();
        let (mut rx, mut tx) = dataset.into_split();

        rx.send(1).await?;
        let x = tx.next().await;
        assert_eq!(x, Some(Ok(1)));

        rx.send(2).await?;
        let x = tx.next().await;
        assert_eq!(x, Some(Ok(2)));

        Ok(())
    }
}
