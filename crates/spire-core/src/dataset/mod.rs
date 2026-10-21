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

mod data;
#[cfg(feature = "serde")]
mod jsonl;
mod memory;
mod registry;

use std::future::Future;

pub use data::Data;
#[cfg(feature = "serde")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
pub use jsonl::JsonlDataset;
pub use memory::InMemDataset;
pub(crate) use registry::Datasets;

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

    /// Returns the number of stored items, if known.
    fn size_hint(&self) -> Option<usize> {
        None
    }
}
