//! Data collection and management with [`Dataset`] and its utilities.
//!
//! This module provides abstractions for storing and retrieving data during web scraping
//! operations. Datasets can be used to queue URLs for processing, store scraped results,
//! or manage any other type of data flow in your scraping pipeline.
//!
//! # Core Concepts
//!
//! ## [`Dataset`] Trait
//!
//! The fundamental trait for expandable collections with async read/write operations.
//! All dataset implementations must provide:
//! - [`write`](Dataset::write) - Add items to the collection
//! - [`read`](Dataset::read) - Remove and return the next item
//! - [`len`](Dataset::len) - Query collection size
//!
//! ## Built-in Implementations
//!
//! - [`InMemDataset`] - Simple in-memory FIFO queue or LIFO stack for fast local storage
//! - Future: `RedbDataset` - Embedded key-value store (planned)
//! - Future: `SqlxDataset` - SQL database backend (planned)
//!
//! ## Type Erasure Utilities ([`DatasetExt`])
//!
//! - [`BoxDataset`] - Type-erased dataset wrapper for heterogeneous collections
//! - [`BoxCloneDataset`] - Cloneable type-erased dataset for shared ownership
//! - [`MapData`] - Transform data types during read/write operations
//! - [`MapErr`] - Convert error types between dataset implementations
//!
//! [`BoxDataset`]: util::BoxDataset
//! [`BoxCloneDataset`]: util::BoxCloneDataset
//! [`MapData`]: util::MapData
//! [`MapErr`]: util::MapErr
//!
//! ## Futures Integration
//!
//! - [`Data`] - Convenient wrapper around [`BoxCloneDataset`] for ergonomic usage
//! - [`DataStream`] - Adapts datasets to `futures::`[`Stream`] for consumption
//! - [`DataSink`] - Adapts datasets to `futures::`[`Sink`] for production
//!
//! [`Stream`]: futures::Stream
//! [`Sink`]: futures::Sink
//! [`Data`]: future::Data
//! [`DataStream`]: future::DataStream
//! [`DataSink`]: future::DataSink
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```ignore
//! use spire_core::dataset::{Dataset, InMemDataset};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a FIFO queue for URLs
//! let urls = InMemDataset::<String>::queue();
//!
//! // Add URLs to process
//! urls.write("https://example.com".to_string()).await?;
//! urls.write("https://example.com/page2".to_string()).await?;
//!
//! // Process URLs in order
//! while let Some(url) = urls.read().await? {
//!     println!("Processing: {}", url);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Using with Futures
//!
//! ```ignore
//! use futures::{SinkExt, StreamExt};
//! use spire_core::dataset::{DatasetExt, InMemDataset};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let dataset = InMemDataset::<i32>::queue();
//! let (mut sink, mut stream) = dataset.into_split();
//!
//! // Producer
//! sink.send(42).await?;
//!
//! // Consumer
//! if let Some(Ok(value)) = stream.next().await {
//!     println!("Received: {}", value);
//! }
//! # Ok(())
//! # }
//! ```

#[doc(inline)]
pub use future::Data;
pub use memory::InMemDataset;
pub(crate) use registry::DatasetRegistry;
#[doc(inline)]
pub use utils::DatasetExt;

pub mod future;
mod memory;
mod registry;
pub mod utils;

/// An expandable, asynchronous collection of items with a queryable size.
///
/// `Dataset` provides a common interface for various storage backends used in
/// web scraping workflows. Implementations can range from simple in-memory queues
/// to persistent database-backed storage.
///
/// # Type Parameters
///
/// - `T` - The type of items stored in this dataset
///
/// # Associated Types
///
/// - [`Error`](Dataset::Error) - The error type for failed operations
///
/// # Thread Safety
///
/// All datasets must be `Send + Sync` to enable concurrent access across async tasks.
///
/// # Examples
///
/// ```ignore
/// use spire_core::dataset::{Dataset, InMemDataset};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let dataset = InMemDataset::<i32>::queue();
///
/// // Write items
/// dataset.write(1).await?;
/// dataset.write(2).await?;
///
/// assert_eq!(dataset.len(), 2);
///
/// // Read items back
/// assert_eq!(dataset.read().await?, Some(1));
/// assert_eq!(dataset.read().await?, Some(2));
/// assert_eq!(dataset.read().await?, None);
/// # Ok(())
/// # }
/// ```
#[async_trait::async_trait]
pub trait Dataset<T>: Send + Sync {
    /// The error type returned by failed [`write`](Dataset::write) or
    /// [`read`](Dataset::read) operations.
    type Error;

    /// Adds an item to the collection.
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`](Dataset::Error) if the write operation fails
    /// (e.g., database connection error, insufficient storage).
    async fn write(&self, data: T) -> Result<(), Self::Error>;

    /// Removes and returns the next item from the collection.
    ///
    /// Returns `None` if the collection is empty.
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`](Dataset::Error) if the read operation fails
    /// (e.g., database connection error, deserialization failure).
    async fn read(&self) -> Result<Option<T>, Self::Error>;

    /// Returns the number of items currently in the dataset.
    ///
    /// Note: For concurrent access patterns, the length may change between
    /// when this is called and when the value is used.
    fn len(&self) -> usize;

    /// Returns `true` if the dataset contains no items.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Extension trait for bulk dataset operations.
///
/// This trait provides efficient batch read/write operations for datasets.
/// It is automatically implemented for all types that implement [`Dataset`].
///
/// # Examples
///
/// ```ignore
/// use spire_core::dataset::{Dataset, DatasetBulkExt, InMemDataset};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let dataset = InMemDataset::<i32>::queue();
///
/// // Write multiple items at once
/// dataset.write_bulk(vec![1, 2, 3, 4, 5]).await?;
///
/// // Read multiple items at once
/// let items = dataset.read_bulk(3).await?;
/// assert_eq!(items, vec![1, 2, 3]);
/// # Ok(())
/// # }
/// ```
#[async_trait::async_trait]
pub trait DatasetBulkExt<T>: Dataset<T>
where
    T: Send + 'static,
{
    /// Writes multiple items to the dataset in bulk.
    ///
    /// The default implementation calls [`write`](Dataset::write) for each item sequentially.
    /// Implementations may override this to provide more efficient bulk operations.
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`](Dataset::Error) if any write operation fails.
    /// On error, some items may have been written before the failure occurred.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_core::dataset::{Dataset, DatasetBulkExt, InMemDataset};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let dataset = InMemDataset::<i32>::queue();
    /// dataset.write_bulk(vec![1, 2, 3, 4, 5]).await?;
    /// assert_eq!(dataset.len(), 5);
    /// # Ok(())
    /// # }
    /// ```
    async fn write_bulk(&self, items: Vec<T>) -> Result<(), Self::Error> {
        for item in items {
            self.write(item).await?;
        }
        Ok(())
    }

    /// Reads multiple items from the dataset in bulk.
    ///
    /// Returns up to `count` items from the dataset. May return fewer items if
    /// the dataset becomes empty before reaching the requested count.
    ///
    /// The default implementation calls [`read`](Dataset::read) up to `count` times.
    /// Implementations may override this to provide more efficient bulk operations.
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`](Dataset::Error) if any read operation fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_core::dataset::{Dataset, DatasetBulkExt, InMemDataset};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let dataset = InMemDataset::<i32>::queue();
    /// dataset.write_bulk(vec![1, 2, 3, 4, 5]).await?;
    ///
    /// let items = dataset.read_bulk(3).await?;
    /// assert_eq!(items, vec![1, 2, 3]);
    /// assert_eq!(dataset.len(), 2);
    /// # Ok(())
    /// # }
    /// ```
    async fn read_bulk(&self, count: usize) -> Result<Vec<T>, Self::Error> {
        let mut items = Vec::with_capacity(count.min(self.len()));
        for _ in 0..count {
            match self.read().await? {
                Some(item) => items.push(item),
                None => break,
            }
        }
        Ok(items)
    }

    /// Reads all items from the dataset.
    ///
    /// Continuously reads items until the dataset is empty. This is equivalent to
    /// calling [`read_bulk`](DatasetBulkExt::read_bulk) with a count equal to the dataset length.
    ///
    /// The default implementation calls [`read`](Dataset::read) until it returns `None`.
    /// Implementations may override this to provide more efficient bulk operations.
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`](Dataset::Error) if any read operation fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use spire_core::dataset::{Dataset, DatasetBulkExt, InMemDataset};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let dataset = InMemDataset::<i32>::queue();
    /// dataset.write_bulk(vec![1, 2, 3]).await?;
    ///
    /// let all_items = dataset.read_all().await?;
    /// assert_eq!(all_items, vec![1, 2, 3]);
    /// assert!(dataset.is_empty());
    /// # Ok(())
    /// # }
    /// ```
    async fn read_all(&self) -> Result<Vec<T>, Self::Error> {
        let mut items = Vec::with_capacity(self.len());
        while let Some(item) = self.read().await? {
            items.push(item);
        }
        Ok(items)
    }
}

/// Blanket implementation of [`DatasetBulkExt`] for all [`Dataset`] types.
impl<T, D> DatasetBulkExt<T> for D
where
    D: Dataset<T>,
    T: Send + 'static,
{
}
