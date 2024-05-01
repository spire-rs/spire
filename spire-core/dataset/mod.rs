//! Data collection with [`Dataset`] and its utilities.
//!
//! - [`Data`] is a [`BoxDataset`] wrapper to avoid the [`Dataset`].
//! - [`DataStream`] is a `futures::`[`Stream`] for `Dataset`s.
//!
//! ### Datasets
//!
//! - [`InMemDataset`] is a simple in-memory `FIFO` or `LIFO` `Dataset`.
//! - `RedbDataset` is an embedded key-value store backed by the `redb` crate.
//! - `SqlxDataset` is an asynchronous `SQL` store backed by the `sqlx` crate.
//!
//! ### Utility
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

use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_stream::try_stream;
use futures::{Stream, StreamExt};
use futures::stream::BoxStream;

pub use memory::InMemDataset;
pub(crate) use sets::Datasets;
#[doc(inline)]
pub use util::DatasetExt;

use crate::{Error, Result};
use crate::dataset::util::BoxCloneDataset;

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

    /// Returns a new [`DataStream`].
    fn into_stream(self) -> DataStream<T, Self::Error>
    where
        Self: Sized,
        T: Send + 'static,
        Self::Error: Send + 'static,
    {
        DataStream::new(self)
    }
}

/// [`DataStream`] is a `futures::`[`Stream`] for [`Dataset`]s.
#[must_use = "streams do nothing unless polled"]
pub struct DataStream<T, E> {
    inner: BoxStream<'static, Result<T, E>>,
}

impl<T, E> DataStream<T, E> {
    fn new<D>(dataset: D) -> Self
    where
        D: Dataset<T, Error = E>,
        T: Send + 'static,
        E: Send + 'static,
    {
        let stream = try_stream! {
            while !dataset.is_empty() {
                let item = dataset.read().await?;
                if let Some(item) = item {
                    yield item;
                }
            }
        };

        DataStream {
            inner: stream.boxed(),
        }
    }
}

impl<T, E> Stream for DataStream<T, E> {
    type Item = Result<T, E>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let pin = Pin::new(&mut self.inner);
        pin.poll_next(cx)
    }
}

/// [`Data`] is a [`BoxCloneDataset`] wrapper to avoid the [`Dataset`].
#[must_use]
#[derive(Clone)]
pub struct Data<T>(pub BoxCloneDataset<T, Error>)
where
    T: 'static;

impl<T> Data<T>
where
    T: Send + Sync + 'static,
{
    /// Creates a new [`Data`].
    #[inline]
    pub fn new(inner: BoxCloneDataset<T, Error>) -> Self {
        Self(inner)
    }

    /// Reads and returns another item from the underlying [`Dataset`].
    #[inline]
    pub async fn read(&self) -> Result<Option<T>> {
        self.0.read().await
    }

    /// Writes another item into the underlying [`Dataset`].
    #[inline]
    pub async fn write(&self, data: T) -> Result<()> {
        self.0.write(data).await
    }

    /// Returns the underlying [`Dataset`].
    #[inline]
    pub fn into_inner(self) -> BoxCloneDataset<T, Error> {
        self.0
    }

    /// Returns a new [`DataStream`].
    #[inline]
    pub fn into_stream(self) -> DataStream<T, Error> {
        self.0.into_stream()
    }
}

impl<T> fmt::Debug for Data<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Dataset").finish_non_exhaustive()
    }
}
