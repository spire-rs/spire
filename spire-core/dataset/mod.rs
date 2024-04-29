//! Data collection with [`Dataset`] and its utilities.
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

pub use memory::InMemDataset;
pub(crate) use ttable::Datasets;
#[doc(inline)]
pub use util::DatasetExt;

mod memory;
mod ttable;
pub mod util;

/// Basic expandable collection of items with a defined size.
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
