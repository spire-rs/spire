//! Datasets: where handlers store what they scrape.
//!
//! A crawl holds at most one [`Dataset`] per item type. Datasets are
//! registered on the [`Crawler`](crate::Crawler) and reached from handlers
//! through a [`Data`] handle:
//!
//! ```
//! use spire_core::Crawler;
//! use spire_core::backend::Noop;
//! use spire_core::dataset::InMemDataset;
//! use spire_core::worker::from_fn;
//! use spire_core::{Context, Result};
//!
//! # async fn example() -> Result<()> {
//! let worker = from_fn(|cx: Context<Noop>| async move {
//!     let titles = cx.dataset::<String>()?;
//!     titles.write("Example Domain".to_owned()).await
//! });
//!
//! let crawler = Crawler::new(Noop, worker).with_dataset(InMemDataset::<String>::new());
//! crawler.run().await?;
//! # Ok(())
//! # }
//! ```

#[cfg(feature = "csv")]
mod csv;
mod data;
#[cfg(any(feature = "json", feature = "csv"))]
mod file;
#[cfg(feature = "json")]
mod jsonl;
mod memory;
mod registry;

use std::future::Future;

pub use data::{Data, READ_BATCH};
#[cfg(feature = "json")]
#[cfg_attr(docsrs, doc(cfg(feature = "json")))]
pub use jsonl::JsonlDataset;
pub use memory::InMemDataset;
pub(crate) use registry::Datasets;

#[cfg(feature = "csv")]
#[cfg_attr(docsrs, doc(cfg(feature = "csv")))]
pub use self::csv::CsvDataset;
use crate::Result;

/// Stores items of type `T`.
pub trait Dataset<T>: Send + Sync + 'static {
    /// Stores an item.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying storage fails.
    fn write(&self, item: T) -> impl Future<Output = Result<()>> + Send;

    /// Takes the next item, or `None` if the dataset is empty.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying storage fails.
    fn read(&self) -> impl Future<Output = Result<Option<T>>> + Send;

    /// Takes up to `max` items at once, in the order [`read`](Self::read)
    /// takes them; fewer, or none, once the dataset runs out.
    ///
    /// The default reads them one by one. Storage that can take several
    /// items at once, such as in a single transaction, overrides it.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying storage fails. Items taken before
    /// the error are lost to the caller.
    fn read_batch(&self, max: usize) -> impl Future<Output = Result<Vec<T>>> + Send
    where
        T: Send,
    {
        async move {
            let mut items = Vec::new();
            while items.len() < max {
                match self.read().await? {
                    Some(item) => items.push(item),
                    None => break,
                }
            }
            Ok(items)
        }
    }

    /// Returns the number of stored items, if known.
    fn size_hint(&self) -> Option<usize> {
        None
    }
}
