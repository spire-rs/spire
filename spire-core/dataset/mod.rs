//! Data collection with [`Dataset`] and its utilities.
//!
//! ### Datasets
//!
//! - [`InMemDataset`]
//! - [`SqliteDataset`]
//! - [`PersyDataset`]
//!
//! ### Utility
//!
//! - [`BoxDataset`]
//! - [`BoxCloneDataset`]
//! - [`MapData`]
//! - [`MapErr`]
//!
//! [`BoxDataset`]: util::BoxDataset
//! [`BoxCloneDataset`]: util::BoxCloneDataset
//! [`MapData`]: util::MapData
//! [`MapErr`]: util::MapErr

pub use memory::InMemDataset;
#[cfg(feature = "persy")]
#[cfg_attr(docsrs, doc(cfg(feature = "persy")))]
pub use persy::PersyDataset;
#[cfg(feature = "sqlite")]
#[cfg_attr(docsrs, doc(cfg(feature = "sqlite")))]
pub use sqlite::SqliteDataset;

mod memory;
#[cfg(feature = "persy")]
mod persy;
#[cfg(feature = "sqlite")]
mod sqlite;
pub mod util;

/// Basic expandable collection of items with a defined size.
///
/// Features a mirrored API from `burn::data::dataset::`[`Dataset`].
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

    /// Checks if the dataset is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
