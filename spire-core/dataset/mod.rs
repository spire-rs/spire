//! Data collection with [`Dataset`] and its utilities.
//!
//! [`Data`] is TODO.
//!
//! ### Datasets
//!
//! - [`InMemDataset`] is a simple in-memory `FIFO` or `LIFO` `VecDeque`-based `Dataset`.
//! - `RedbDataset`
//! - `PersyDataset`
//! - `SqliteDataset`
//! - `SqlxDataset`
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

pub use memory::InMemDataset;
pub(crate) use sets::Datasets;
#[doc(inline)]
pub use util::DatasetExt;

use crate::dataset::util::BoxCloneDataset;
use crate::{Error, Result};

mod memory;
mod sets;
pub mod util;

/// Expandable collection of items with a defined size.
///
/// Features a mirrored async API from `burn::data::dataset::`[`Dataset`].
///
/// [`Dataset`]: https://docs.rs/burn/0.12.1/burn/data/dataset/trait.Dataset.html
#[async_trait::async_trait]
pub trait Dataset<T>: Send + Sync + 'static {
    type Error;

    /// Inserts another item into the collection.
    async fn add(&self, data: T) -> Result<(), Self::Error>;

    /// Removes and returns the next item from the collection.
    async fn get(&self) -> Result<Option<T>, Self::Error>;

    /// Returns the number of items in the dataset.
    fn len(&self) -> usize;

    /// Returns `true` if the dataset is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// TODO.
#[derive(Clone)]
pub struct Data<T>(BoxCloneDataset<T, Error>)
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

    /// TODO.
    #[inline]
    pub async fn read(&self) -> Result<Option<T>> {
        self.0.get().await
    }

    /// TODO.
    #[inline]
    pub async fn write(&self, data: T) -> Result<()> {
        self.0.add(data).await
    }

    /// TODO.
    #[inline]
    pub fn into_inner(self) -> BoxCloneDataset<T, Error> {
        self.0
    }
}

impl<T> fmt::Debug for Data<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Dataset").finish_non_exhaustive()
    }
}
