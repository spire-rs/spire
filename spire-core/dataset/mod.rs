pub use boxed::BoxDataset;
pub use memory::InMemDataset;
pub use sqlite::SqliteDataset;

mod boxed;
mod memory;
mod sets;
mod sqlite;

/// Unrecoverable failure during [`Dataset`] insertion.
///
/// This may be extended in the future so exhaustive matching is discouraged.
#[derive(Debug, Default)]
pub enum Error {
    Duplicate,
    OutOfSpace,
    #[default]
    Unknown,
}

/// A specialized [`Result`] type for [`Dataset`] insertion.
///
/// [`Result`]: std::result::Result
pub type Result<T> = std::result::Result<T, Error>;

/// Defines a basic expandable collection of items with a defined size.
///
/// Features a mirrored API from `burn::data::dataset::`[`Dataset`].
///
/// [`Dataset`]: https://docs.rs/burn/0.12.1/burn/data/dataset/trait.Dataset.html
pub trait Dataset<T> {
    /// Inserts another item into the collection.
    fn append(&self, data: T) -> Result<()>;

    fn evict(&self) -> Option<T>;

    /// Returns the number of items in the dataset.
    fn len(&self) -> usize;

    /// Checks if the dataset is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
