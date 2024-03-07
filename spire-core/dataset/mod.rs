pub use boxed::BoxDataset;
pub use memory::InMemDataset;

mod boxed;
mod memory;

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
#[async_trait::async_trait]
pub trait Dataset<T>: Send + Sync + 'static {
    /// Inserts another item into the collection.
    async fn append(&self, data: T) -> Result<()>;

    /// Removes and returns the next item from the collection.
    async fn evict(&self) -> Option<T>;

    /// Returns the number of items in the dataset.
    fn len(&self) -> usize;

    /// Checks if the dataset is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
