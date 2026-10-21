use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use redb::{ReadableTable, TableDefinition};
use serde::Serialize;
use serde::de::DeserializeOwned;
use spire_core::dataset::Dataset;
use spire_core::{Error, ErrorKind, Result};

use crate::store::RedbStore;

/// A [`Dataset`] stored in a [`RedbStore`], as a table next to the
/// frontier, so one file holds the whole crawl.
///
/// Items are stored as JSON, in the order they were written, and read back
/// first in, first out: [`read`](Dataset::read) takes the oldest item and
/// removes it. Clones share the table.
///
/// Each item is committed as it is written, and synced to disk with the
/// store's other changes, so items survive a crash as the frontier does.
/// Items and the frontier are committed separately, though: a page whose
/// items were committed just before a crash may be crawled again, writing
/// them twice. Deduplicate by a key such as the URL when reading.
///
/// # Examples
///
/// ```no_run
/// use spire_core::dataset::Dataset;
/// use spire_redb::RedbStore;
///
/// #[derive(serde::Serialize, serde::Deserialize)]
/// struct Product {
///     name: String,
///     price: f64,
/// }
///
/// # async fn example() -> spire_core::Result<()> {
/// let store = RedbStore::open("crawl.redb").await?;
/// let products = store.dataset::<Product>("products");
/// products.write(Product { name: "Lamp".into(), price: 20.0 }).await?;
/// # Ok(())
/// # }
/// ```
pub struct RedbDataset<T> {
    store: RedbStore,
    table: Arc<str>,
    marker: PhantomData<fn(T) -> T>,
}

impl<T> RedbDataset<T> {
    pub(crate) fn new(store: RedbStore, name: &str) -> Self {
        Self {
            store,
            table: format!("dataset:{name}").into(),
            marker: PhantomData,
        }
    }

    /// Returns the number of items stored.
    ///
    /// # Errors
    ///
    /// Returns a [`Dataset`](ErrorKind::Dataset) error if the store cannot be
    /// read.
    pub fn len(&self) -> Result<usize> {
        self.store
            .count(definition(&self.table))
            .map_err(dataset_error)
    }

    /// Whether no item is stored.
    ///
    /// # Errors
    ///
    /// Returns a [`Dataset`](ErrorKind::Dataset) error if the store cannot be
    /// read.
    pub fn is_empty(&self) -> Result<bool> {
        self.len().map(|len| len == 0)
    }
}

/// The table of a dataset: items by insertion order.
fn definition(name: &str) -> TableDefinition<'_, u64, &'static [u8]> {
    TableDefinition::new(name)
}

impl<T> Clone for RedbDataset<T> {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            table: Arc::clone(&self.table),
            marker: PhantomData,
        }
    }
}

impl<T> fmt::Debug for RedbDataset<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedbDataset")
            .field("store", &self.store)
            .field("table", &self.table)
            .finish_non_exhaustive()
    }
}

impl<T> Dataset<T> for RedbDataset<T>
where
    T: Serialize + DeserializeOwned + Send + 'static,
{
    async fn write(&self, item: T) -> Result<()> {
        let bytes = serde_json::to_vec(&item).map_err(|error| {
            Error::new(ErrorKind::Dataset)
                .with_message("failed to encode an item")
                .with_source(error)
        })?;
        let name = Arc::clone(&self.table);
        self.store
            .write(move |txn| {
                let mut table = txn.open_table(definition(&name))?;
                let next = table.last()?.map_or(0, |(key, _)| key.value() + 1);
                table.insert(next, bytes.as_slice())?;
                Ok(())
            })
            .await
            .map_err(dataset_error)
    }

    /// Takes the oldest item. An item that no longer decodes as `T`, e.g.
    /// after the type changed, is removed and returned as an error.
    async fn read(&self) -> Result<Option<T>> {
        // A read transaction is enough to find the table empty, as the last
        // read of a drain does, without committing a write.
        if self.len()? == 0 {
            return Ok(None);
        }
        let name = Arc::clone(&self.table);
        let bytes = self
            .store
            .write(move |txn| {
                let mut table = txn.open_table(definition(&name))?;
                let first = table.pop_first()?;
                Ok(first.map(|(_, bytes)| bytes.value().to_vec()))
            })
            .await
            .map_err(dataset_error)?;

        bytes.map(|bytes| decode(&bytes)).transpose()
    }

    /// Takes up to `max` items in one transaction. An item that no longer
    /// decodes ends the batch before it, so no item decoded before it is
    /// lost; taking it first, the next batch returns it as an error.
    async fn read_batch(&self, max: usize) -> Result<Vec<T>> {
        if max == 0 || self.len()? == 0 {
            return Ok(Vec::new());
        }
        let name = Arc::clone(&self.table);
        self.store
            .write(move |txn| {
                let mut table = txn.open_table(definition(&name))?;
                let mut items = Vec::new();
                while items.len() < max {
                    let Some((key, bytes)) = table
                        .pop_first()?
                        .map(|(key, bytes)| (key.value(), bytes.value().to_vec()))
                    else {
                        break;
                    };
                    match decode(&bytes) {
                        Ok(item) => items.push(item),
                        Err(error) if items.is_empty() => return Ok(Err(error)),
                        Err(_) => {
                            table.insert(key, bytes.as_slice())?;
                            break;
                        }
                    }
                }
                Ok(Ok(items))
            })
            .await
            .map_err(dataset_error)?
    }

    fn size_hint(&self) -> Option<usize> {
        self.len().ok()
    }
}

/// Decodes a stored item.
fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    serde_json::from_slice(bytes).map_err(|error| {
        Error::new(ErrorKind::Dataset)
            .with_message("failed to decode an item")
            .with_source(error)
    })
}

/// Reports a failed store operation as a dataset error.
fn dataset_error(error: Error) -> Error {
    Error::new(ErrorKind::Dataset)
        .with_message("redb storage failed")
        .with_source(error)
}

#[cfg(test)]
mod tests {
    use spire_core::dataset::Data;

    use super::*;
    use crate::frontier::tests::TempPath;

    #[derive(Debug, PartialEq, Serialize, serde::Deserialize)]
    struct Page {
        url: String,
        words: u32,
    }

    fn page(url: &str, words: u32) -> Page {
        Page {
            url: url.to_owned(),
            words,
        }
    }

    #[tokio::test]
    async fn reads_items_in_the_order_written_and_survives_reopening() {
        let path = TempPath::new();
        {
            let store = RedbStore::open(&path.0).await.unwrap();
            let pages = store.dataset::<Page>("pages");
            assert_eq!(pages.size_hint(), Some(0));
            pages.write(page("/a", 3)).await.unwrap();
            pages.clone().write(page("/b", 5)).await.unwrap();
            assert_eq!(pages.read().await.unwrap(), Some(page("/a", 3)));
            pages.write(page("/c", 8)).await.unwrap();
        }

        let store = RedbStore::open(&path.0).await.unwrap();
        let pages = store.dataset::<Page>("pages");
        assert_eq!(pages.len().unwrap(), 2);
        let all = Data::new(pages.clone()).read_all().await.unwrap();
        assert_eq!(all, [page("/b", 5), page("/c", 8)]);
        assert!(pages.is_empty().unwrap());
        assert_eq!(pages.read().await.unwrap(), None);
    }

    #[tokio::test]
    async fn names_are_separate_tables() {
        let path = TempPath::new();
        let store = RedbStore::open(&path.0).await.unwrap();
        store.dataset::<u32>("a").write(1).await.unwrap();
        store.dataset::<u32>("b").write(2).await.unwrap();

        assert_eq!(store.dataset::<u32>("a").read().await.unwrap(), Some(1));
        assert_eq!(store.dataset::<u32>("b").read().await.unwrap(), Some(2));
        // The frontier's tables are untouched.
        assert_eq!(store.frontier().in_flight().unwrap(), 0);
    }

    #[tokio::test]
    async fn items_that_no_longer_decode_are_errors() {
        let path = TempPath::new();
        let store = RedbStore::open(&path.0).await.unwrap();
        store
            .dataset::<String>("items")
            .write("text".to_owned())
            .await
            .unwrap();
        store
            .dataset::<String>("items")
            .write("more".to_owned())
            .await
            .unwrap();

        let numbers = store.dataset::<u32>("items");
        let error = numbers.read().await.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Dataset);
        // The bad item is gone, so reading continues with the next.
        assert_eq!(
            store
                .dataset::<String>("items")
                .read()
                .await
                .unwrap()
                .as_deref(),
            Some("more")
        );
    }

    #[tokio::test]
    async fn batches_take_several_items_in_order() {
        let path = TempPath::new();
        let store = RedbStore::open(&path.0).await.unwrap();
        let numbers = store.dataset::<u32>("numbers");
        Data::new(numbers.clone()).write_all(0..5).await.unwrap();

        assert_eq!(numbers.read_batch(3).await.unwrap(), [0, 1, 2]);
        assert_eq!(numbers.read_batch(3).await.unwrap(), [3, 4]);
        assert_eq!(numbers.read_batch(3).await.unwrap(), Vec::<u32>::new());
        assert_eq!(
            store
                .dataset::<u32>("never_written")
                .read_batch(3)
                .await
                .unwrap(),
            Vec::<u32>::new()
        );
    }

    #[tokio::test]
    async fn batches_stop_before_items_that_no_longer_decode() {
        let path = TempPath::new();
        let store = RedbStore::open(&path.0).await.unwrap();
        let text = store.dataset::<String>("mixed");
        let numbers = store.dataset::<u32>("mixed");
        numbers.write(1).await.unwrap();
        text.write("two".to_owned()).await.unwrap();
        numbers.write(3).await.unwrap();

        assert_eq!(numbers.read_batch(10).await.unwrap(), [1]);
        assert!(numbers.read_batch(10).await.is_err());
        assert_eq!(numbers.read_batch(10).await.unwrap(), [3]);
    }
}
