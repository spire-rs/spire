use std::pin::Pin;
use std::task::{Context, Poll};

use futures::stream::{unfold, BoxStream};
use futures::{Stream, StreamExt};

use crate::dataset::Dataset;
use crate::{Error, Result};

///`futures::`[`Stream`] for [`Dataset`]s.
///
/// See [`Dataset::into_stream`] and [`Data::into_stream`].
///
/// [`Data::into_stream`]: crate::dataset::Data::into_stream
#[must_use = "streams do nothing unless you poll them"]
pub struct DataStream<T, E = Error> {
    inner: BoxStream<'static, Result<T, E>>,
}

impl<T, E> DataStream<T, E> {
    /// Creates a new [`DataStream`].
    pub(crate) fn new<D>(dataset: D) -> Self
    where
        D: Dataset<T, Error = E>,
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
