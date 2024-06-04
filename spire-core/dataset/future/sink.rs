use std::pin::Pin;
use std::task::{Context, Poll};

use futures::sink::unfold;
use futures::{Sink, SinkExt};

use crate::dataset::Dataset;
use crate::Error;

/// Idk why it's not a part of `futures`.
type BoxSink<'a, T, E> = Pin<Box<dyn Sink<T, Error = E> + Send + 'a>>;

/// `futures::`[`Sink`] for [`Dataset`]s.
///
/// See [`Dataset::into_sink`] and [`Data::into_sink`].
///
/// [`Data::into_sink`]: crate::dataset::Data::into_sink
#[must_use = "sinks do nothing unless you poll them"]
pub struct DataSink<T, E = Error> {
    inner: BoxSink<'static, T, E>,
}

impl<T, E> DataSink<T, E> {
    /// Creates a new [`DataSink`].
    pub(crate) fn new<D>(dataset: D) -> Self
    where
        D: Dataset<T, Error = E>,
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
