//! Data collection with [`Dataset`] and its utilities.
//!
//! ### [`Dataset`]s
//!
//! - [`InMemDataset`] is a simple in-memory `FIFO` or `LIFO` `Dataset`.
//! - `RedbDataset` is an embedded key-value store backed by the `redb` crate.
//! - `SqlxDataset` is an asynchronous `SQL` store backed by the `sqlx` crate.
//!
//! ### [`DatasetExt`] utilities
//!
//! - [`BoxDataset`] is a type-erased `Dataset`.
//! - [`BoxCloneDataset`] is a cloneable type-erased `Dataset`.
//! - [`MapData`] transforms the data type of the `Dataset`.
//! - [`MapErr`] transforms the error type of the `Dataset`.
//!
//! [`BoxDataset`]: util::BoxDataset
//! [`BoxCloneDataset`]: util::BoxCloneDataset
//! [`MapData`]: util::MapData
//! [`MapErr`]: util::MapErr
//!
//! ### [`Stream`]s and [`Sink`]s
//!
//! - [`Data`] is a convenience [`BoxCloneDataset`] wrapper.
//! - [`DataStream`] is a `futures::`[`Stream`] for `Dataset`s.
//! - [`DataSink`] is a `futures::`[`Sink`] for `Dataset`s.
//!
//! [`Stream`]: futures::Stream
//! [`Sink`]: futures::Sink
//!
//! [`Data`]: future::Data
//! [`DataStream`]: future::DataStream
//! [`DataSink`]: future::DataSink

#[doc(inline)]
pub use future::Data;
pub use memory::InMemDataset;
pub(crate) use sets::Datasets;
#[doc(inline)]
pub use util::DatasetExt;

use crate::dataset::future::{DataSink, DataStream};

pub mod future;
mod memory;
mod sets;
pub mod util;

/// Expandable collection of items with a defined size.
#[async_trait::async_trait]
pub trait Dataset<T>: Send + Sync + 'static {
    /// Unrecoverable `Dataset` failure.
    type Error;

    /// Writes another item into the collection.
    async fn write(&self, data: T) -> Result<(), Self::Error>;

    /// Reads and returns the next item from the collection.
    async fn read(&self) -> Result<Option<T>, Self::Error>;

    /// Returns the number of items in the dataset.
    fn len(&self) -> usize;

    /// Returns `true` if the dataset is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns both [`DataSink`] and [`DataStream`].
    fn into_split(self) -> (DataSink<T, Self::Error>, DataStream<T, Self::Error>)
    where
        Self: Sized + Clone,
        T: Send + 'static,
        Self::Error: Send + 'static,
    {
        (self.clone().into_sink(), self.into_stream())
    }

    /// Returns a new [`DataStream`].
    fn into_stream(self) -> DataStream<T, Self::Error>
    where
        Self: Sized,
        T: Send + 'static,
        Self::Error: Send + 'static,
    {
        DataStream::new(self)
    }

    /// Returns a new [`DataSink`].
    fn into_sink(self) -> DataSink<T, Self::Error>
    where
        Self: Sized,
        T: Send + 'static,
        Self::Error: Send + 'static,
    {
        DataSink::new(self)
    }
}
