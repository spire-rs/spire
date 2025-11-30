use std::pin::Pin;
use std::task::{Context, Poll};

use futures::stream::{BoxStream, unfold};
use futures::{Stream, StreamExt};

use crate::dataset::Dataset;
use crate::{Error, Result};

/// A `futures::`[`Stream`] adapter for [`Dataset`]s.
///
/// `DataStream` allows you to use datasets as streams in futures-based async code,
/// enabling seamless integration with stream processing pipelines.
///
/// Items are pulled from the underlying dataset using [`Dataset::read`].
/// The stream terminates when the dataset returns `None`, indicating no more
/// items are available.
///
/// # Creation
///
/// Create a `DataStream` using [`DatasetExt::into_stream`] or [`DatasetExt::into_split`]:
///
/// ```ignore
/// use futures::StreamExt;
/// use spire_core::dataset::{Dataset, DatasetExt, InMemDataset};
///
/// # async fn example() -> Result<(), std::convert::Infallible> {
/// let dataset = InMemDataset::<i32>::queue();
/// dataset.write(1).await?;
/// dataset.write(2).await?;
/// dataset.write(3).await?;
///
/// let mut stream = dataset.into_stream();
/// while let Some(Ok(item)) = stream.next().await {
///     println!("Item: {}", item);
/// }
/// # Ok(())
/// # }
/// ```
///
/// # Stream Processing
///
/// ```ignore
/// use futures::StreamExt;
/// use spire_core::dataset::{DatasetExt, InMemDataset};
///
/// # async fn example() -> Result<(), std::convert::Infallible> {
/// let dataset = InMemDataset::<i32>::queue();
/// dataset.write(1).await?;
/// dataset.write(2).await?;
/// dataset.write(3).await?;
///
/// let sum: i32 = dataset.into_stream()
///     .filter_map(|result| async move { result.ok() })
///     .fold(0, |acc, x| async move { acc + x })
///     .await;
///
/// println!("Sum: {}", sum);
/// # Ok(())
/// # }
/// ```
///
/// [`DatasetExt::into_stream`]: crate::dataset::DatasetExt::into_stream
/// [`DatasetExt::into_split`]: crate::dataset::DatasetExt::into_split
#[must_use = "streams do nothing unless you poll them"]
pub struct DataStream<T, E = Error> {
    inner: BoxStream<'static, Result<T, E>>,
}

impl<T, E> DataStream<T, E> {
    /// Creates a new [`DataStream`].
    pub(crate) fn new<D>(dataset: D) -> Self
    where
        D: Dataset<T, Error = E> + 'static,
        T: Send + 'static,
        E: Send + 'static,
    {
        let stream = unfold(dataset, |dataset| async move {
            let x = dataset.read().await.transpose();
            x.map(|x| (x, dataset))
        });

        Self {
            inner: stream.boxed(),
        }
    }
}

impl<T, E> Stream for DataStream<T, E> {
    type Item = Result<T, E>;

    #[inline]
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.poll_next_unpin(cx)
    }
}
