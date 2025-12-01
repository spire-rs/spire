use std::pin::Pin;
use std::task::{Context, Poll};

use futures::sink::unfold;
use futures::{Sink, SinkExt};

use crate::Error;
use crate::dataset::Dataset;

/// Type alias for boxed sinks to reduce repetition.
type BoxSink<'a, T, E> = Pin<Box<dyn Sink<T, Error = E> + Send + 'a>>;

/// A `futures::`[`Sink`] adapter for [`Dataset`]s.
///
/// `DataSink` allows you to use datasets as sinks in futures-based async code,
/// enabling seamless integration with stream processing pipelines.
///
/// Items sent to this sink are written to the underlying dataset using
/// [`Dataset::write`]. The sink implementation handles buffering and
/// error propagation automatically.
///
/// # Creation
///
/// Create a `DataSink` using [`DatasetExt::into_sink`] or [`DatasetExt::into_split`]:
///
/// ```ignore
/// use futures::SinkExt;
/// use spire_core::dataset::{DatasetExt, InMemDataset};
///
/// # async fn example() -> Result<(), std::convert::Infallible> {
/// let dataset = InMemDataset::<i32>::queue();
/// let mut sink = dataset.into_sink();
///
/// sink.send(42).await?;
/// sink.send(100).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Producer-Consumer Pattern
///
/// ```ignore
/// use futures::SinkExt;
/// use spire_core::dataset::{DatasetExt, InMemDataset};
///
/// # async fn example() -> Result<(), std::convert::Infallible> {
/// let dataset = InMemDataset::<String>::queue();
/// let (mut sink, stream) = dataset.into_split();
///
/// // Producer task
/// tokio::spawn(async move {
///     for i in 0..10 {
///         sink.send(format!("item-{}", i)).await.unwrap();
///     }
/// });
///
/// // Consumer uses the stream...
/// # Ok(())
/// # }
/// ```
///
/// [`DatasetExt::into_sink`]: crate::dataset::DatasetExt::into_sink
/// [`DatasetExt::into_split`]: crate::dataset::DatasetExt::into_split
#[must_use = "sinks do nothing unless you poll them"]
pub struct DataSink<T, E = Error> {
    inner: BoxSink<'static, T, E>,
}

impl<T, E> DataSink<T, E> {
    /// Creates a new [`DataSink`].
    pub(crate) fn new<D>(dataset: D) -> Self
    where
        D: Dataset<T, Error = E> + 'static,
        T: Send + 'static,
        E: Send + 'static,
    {
        let sink = unfold(dataset, |dataset, data| async move {
            dataset.write(data).await?;
            Ok::<D, E>(dataset)
        });

        let inner: BoxSink<'static, T, E> = Box::pin(sink);
        Self { inner }
    }
}

impl<T, E> Sink<T> for DataSink<T, E> {
    type Error = E;

    #[inline]
    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready_unpin(cx)
    }

    #[inline]
    fn start_send(mut self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        self.inner.start_send_unpin(item)
    }

    #[inline]
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_flush_unpin(cx)
    }

    #[inline]
    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_close_unpin(cx)
    }
}
