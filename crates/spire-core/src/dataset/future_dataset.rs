use std::fmt;
use std::pin::Pin;
use std::sync::Arc;

use futures::{Sink, SinkExt, Stream, StreamExt};
use tokio::sync::Mutex;

use crate::dataset::Dataset;

/// Type aliases for boxed stream and sink to reduce repetition.
type BoxStream<T, E> = Pin<Box<dyn Stream<Item = Result<T, E>> + Send + Sync>>;
type BoxSink<T, E> = Pin<Box<dyn Sink<T, Error = E> + Send + Sync>>;

/// Dataset implementation using separate stream and sink components.
///
/// `FutureDataset` provides a dataset interface backed by arbitrary `Stream` and `Sink`
/// implementations, allowing integration with external data sources and destinations.
///
/// # Type Parameters
///
/// - `T` - The type of items stored/retrieved
/// - `E` - The error type for operations
///
/// # Examples
///
/// ```no_run
/// use futures::channel::mpsc;
/// use futures::{SinkExt, StreamExt};
/// use spire_core::dataset::{Dataset, FutureDataset};
///
/// # async fn example() -> spire_core::Result<()> {
/// let (tx, rx) = mpsc::unbounded::<i32>();
/// let (tx2, rx2) = mpsc::unbounded::<i32>();
///
/// let dataset = FutureDataset::new(
///     rx.map(Ok),
///     tx.sink_map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("sink error: {}", e)))
/// );
///
/// // Use dataset like any other
/// dataset.write(42).await?;
/// let value = dataset.read().await?;
/// # Ok(())
/// # }
/// ```
pub struct FutureDataset<T, E> {
    stream: Arc<Mutex<BoxStream<T, E>>>,
    sink: Arc<Mutex<BoxSink<T, E>>>,
}

impl<T, E> FutureDataset<T, E>
where
    T: Send + Sync + 'static,
    E: Send + Sync + 'static,
{
    /// Creates a new `FutureDataset` with the given stream and sink.
    ///
    /// # Arguments
    ///
    /// - `stream` - Stream to read items from
    /// - `sink` - Sink to write items to
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use futures::channel::mpsc;
    /// use futures::{SinkExt, StreamExt};
    /// use spire_core::dataset::FutureDataset;
    ///
    /// let (tx, rx) = mpsc::unbounded::<i32>();
    /// let (tx2, rx2) = mpsc::unbounded::<i32>();
    ///
    /// let dataset = FutureDataset::new(
    ///     rx.map(Ok),
    ///     tx.sink_map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("error: {}", e)))
    /// );
    /// ```
    pub fn new<S, K>(stream: S, sink: K) -> Self
    where
        S: Stream<Item = Result<T, E>> + Send + Sync + 'static,
        K: Sink<T, Error = E> + Send + Sync + 'static,
    {
        Self {
            stream: Arc::new(Mutex::new(Box::pin(stream))),
            sink: Arc::new(Mutex::new(Box::pin(sink))),
        }
    }
}

#[async_trait::async_trait]
impl<T, E> Dataset<T> for FutureDataset<T, E>
where
    T: Send + Sync + 'static,
    E: Send + Sync + 'static,
{
    type Error = E;

    async fn write(&self, data: T) -> Result<(), Self::Error> {
        let mut sink = self.sink.lock().await;
        sink.send(data).await
    }

    async fn read(&self) -> Result<Option<T>, Self::Error> {
        let mut stream = self.stream.lock().await;
        match stream.next().await {
            Some(result) => result.map(Some),
            None => Ok(None),
        }
    }

    fn len(&self) -> usize {
        // For stream/sink based datasets, length is not easily determinable
        // without consuming the stream, so we return 0
        0
    }
}

impl<T, E> Clone for FutureDataset<T, E> {
    fn clone(&self) -> Self {
        Self {
            stream: Arc::clone(&self.stream),
            sink: Arc::clone(&self.sink),
        }
    }
}

impl<T, E> fmt::Debug for FutureDataset<T, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FutureDataset").finish_non_exhaustive()
    }
}
