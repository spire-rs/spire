use std::fmt;
use std::future::{Future, ready};
use std::marker::PhantomData;
use std::path::Path;

use serde::Serialize;

use crate::dataset::Dataset;
use crate::dataset::file::FileSink;
use crate::{Error, ErrorKind, Result};

/// A write-only [`Dataset`] that appends every item to a CSV file as one
/// row, with a header row of the item's field names.
///
/// Items must be flat: structs or tuples of strings, numbers, booleans and
/// `Option`s of them, as the [csv](https://docs.rs/csv) crate serializes
/// them. Every row must have the columns of the first: an item with a nested
/// struct, a sequence or a map, or with other fields than the rows before
/// it, fails to write. [`append`](Self::append) checks items against the
/// file's existing header.
///
/// Each row is written and flushed as it is stored, under a lock, so rows
/// never interleave; a write that fails partway is cut off, so the file
/// never holds half a row. A flush hands the row to the operating system
/// without syncing it to disk: rows survive a crash of the crawl, not of the
/// machine. Reading returns nothing; read the file instead. Clones share
/// the file, which is locked until the last clone is dropped: opening it
/// again, in this process or another, fails.
///
/// Writes are not part of the frontier's state, so after a crash a request
/// whose items were written may be crawled again, writing them twice: dedupe
/// by a key such as the URL when reading the file.
///
/// ```no_run
/// use spire_core::dataset::CsvDataset;
///
/// #[derive(serde::Serialize)]
/// struct Product {
///     name: String,
///     price: Option<f64>,
/// }
///
/// # async fn example() -> spire_core::Result<()> {
/// let products = CsvDataset::<Product>::create("products.csv").await?;
/// # Ok(())
/// # }
/// ```
pub struct CsvDataset<T> {
    sink: FileSink<Columns>,
    marker: PhantomData<fn(T)>,
}

/// The columns rows must have.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Columns {
    /// Nothing is written yet: the first row decides.
    New,
    /// The file already starts with this line, a header or a row.
    Existing(Vec<u8>),
    /// Rows are of structs, with this header row.
    Header(Vec<u8>),
    /// Rows have no header, and this many fields.
    Fields(usize),
}

impl<T> CsvDataset<T> {
    /// Creates the file, truncating it if it exists. The header row is
    /// written with the first item.
    ///
    /// # Errors
    ///
    /// Returns a [`Dataset`](ErrorKind::Dataset) error if the file cannot be
    /// created, or is open by another dataset.
    pub async fn create(path: impl AsRef<Path>) -> Result<Self> {
        let sink = FileSink::create(path.as_ref(), Columns::New).await?;
        Ok(Self::from_sink(sink))
    }

    /// Opens the file to append to it, creating it if it does not exist,
    /// e.g. to resume a crawl. A header row is written only if the file is
    /// empty; otherwise items must match the file's first line.
    ///
    /// # Errors
    ///
    /// Returns a [`Dataset`](ErrorKind::Dataset) error if the file cannot be
    /// opened or read, or is open by another dataset.
    pub async fn append(path: impl AsRef<Path>) -> Result<Self> {
        let sink = FileSink::append(path.as_ref(), |first| {
            if first.is_empty() {
                Columns::New
            } else {
                Columns::Existing(first.to_vec())
            }
        })
        .await?;
        Ok(Self::from_sink(sink))
    }

    const fn from_sink(sink: FileSink<Columns>) -> Self {
        Self {
            sink,
            marker: PhantomData,
        }
    }

    /// Returns the path of the file.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.sink.path()
    }
}

/// Encodes `item` as a row with `columns`, returning the bytes to append
/// and the columns of the rows after it.
fn encode<T: Serialize>(item: &T, columns: &Columns) -> Result<(Vec<u8>, Columns)> {
    let serialize_error = |error: csv::Error| {
        Error::new(ErrorKind::Dataset)
            .with_message("failed to serialize an item as a CSV row")
            .with_source(error)
    };
    let row = serialize(item, false).map_err(serialize_error)?;
    // Only serializing a header rejects nested fields, which are otherwise
    // flattened into extra columns.
    let full = serialize(item, true).map_err(serialize_error)?;
    let header = full[..full.len() - row.len()].to_vec();
    let fields = count_fields(&row);

    let mismatch = || {
        Error::new(ErrorKind::Dataset)
            .with_message("the item's columns do not match the rows before it")
    };
    let next = match columns {
        Columns::New if header.is_empty() => Some(Columns::Fields(fields)),
        Columns::New => return Ok((full, Columns::Header(header))),
        Columns::Existing(first) if header.is_empty() => {
            (count_fields(first) == fields).then_some(Columns::Fields(fields))
        }
        Columns::Existing(first) => (*first == header).then_some(Columns::Header(header)),
        Columns::Header(expected) => (*expected == header).then(|| columns.clone()),
        Columns::Fields(expected) => {
            (header.is_empty() && *expected == fields).then(|| columns.clone())
        }
    };
    next.map(|next| (row, next)).ok_or_else(mismatch)
}

/// Serializes `item` as a CSV row, preceded by its header row if `header`
/// and it has one.
fn serialize<T: Serialize>(item: &T, header: bool) -> csv::Result<Vec<u8>> {
    let mut rows = csv::WriterBuilder::new()
        .has_headers(header)
        .buffer_capacity(256)
        .from_writer(Vec::new());
    rows.serialize(item)?;
    rows.into_inner().map_err(|error| error.into_error().into())
}

/// Counts the fields of a CSV row.
fn count_fields(row: &[u8]) -> usize {
    csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(row)
        .records()
        .next()
        .and_then(Result::ok)
        .map_or(0, |record| record.len())
}

impl<T> Clone for CsvDataset<T> {
    fn clone(&self) -> Self {
        Self::from_sink(self.sink.clone())
    }
}

impl<T> fmt::Debug for CsvDataset<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CsvDataset")
            .field("path", &self.path())
            .field("written", &self.sink.written())
            .finish()
    }
}

impl<T> Dataset<T> for CsvDataset<T>
where
    T: Serialize + Send + 'static,
{
    async fn write(&self, item: T) -> Result<()> {
        self.sink.write(move |columns| encode(&item, columns)).await
    }

    /// Always returns `None`: the dataset is write-only.
    fn read(&self) -> impl Future<Output = Result<Option<T>>> + Send {
        ready(Ok(None))
    }

    fn size_hint(&self) -> Option<usize> {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[derive(Serialize)]
    struct Product {
        name: &'static str,
        price: Option<f64>,
    }

    #[derive(Serialize)]
    struct Nested {
        name: &'static str,
        tags: Vec<&'static str>,
    }

    fn temp_path(name: &str) -> PathBuf {
        let unique = format!("spire-{name}-{}.csv", std::process::id());
        std::env::temp_dir().join(unique)
    }

    #[tokio::test]
    async fn writes_a_header_then_one_row_per_item_and_appends() {
        let path = temp_path("csv");
        let products = CsvDataset::<Product>::create(&path).await.unwrap();
        let lamp = Product {
            name: "Lamp, desk",
            price: Some(20.5),
        };
        products.write(lamp).await.unwrap();
        let chair = Product {
            name: "Chair",
            price: None,
        };
        products.clone().write(chair).await.unwrap();
        assert!(products.read().await.unwrap().is_none());

        // Appending to a file with rows writes no second header.
        drop(products);
        let resumed = CsvDataset::<Product>::append(&path).await.unwrap();
        let shelf = Product {
            name: "Shelf",
            price: Some(7.0),
        };
        resumed.write(shelf).await.unwrap();

        let contents = tokio::fs::read_to_string(&path).await.unwrap();
        tokio::fs::remove_file(&path).await.unwrap();
        assert_eq!(
            contents,
            "name,price\n\"Lamp, desk\",20.5\nChair,\nShelf,7.0\n"
        );
    }

    #[tokio::test]
    async fn appending_to_a_new_file_writes_a_header() {
        let path = temp_path("csv-new");
        let _ = tokio::fs::remove_file(&path).await;
        let products = CsvDataset::<Product>::append(&path).await.unwrap();
        let lamp = Product {
            name: "Lamp",
            price: Some(1.0),
        };
        products.write(lamp).await.unwrap();

        let contents = tokio::fs::read_to_string(&path).await.unwrap();
        tokio::fs::remove_file(&path).await.unwrap();
        assert_eq!(contents, "name,price\nLamp,1.0\n");
    }

    #[tokio::test]
    async fn nested_items_fail_without_corrupting_the_file() {
        let path = temp_path("csv-nested");
        let items = CsvDataset::<Nested>::create(&path).await.unwrap();
        let nested = Nested {
            name: "Lamp",
            tags: vec!["new"],
        };
        let error = items.write(nested).await.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Dataset);

        let contents = tokio::fs::read_to_string(&path).await.unwrap();
        tokio::fs::remove_file(&path).await.unwrap();
        assert_eq!(contents, "");
    }

    #[tokio::test]
    async fn nested_or_different_items_after_the_first_fail() {
        let path = temp_path("csv-mixed");
        let items = CsvDataset::<Nested>::create(&path).await.unwrap();
        let flat = CsvDataset::<Product>::from_sink(items.sink.clone());
        let lamp = Product {
            name: "Lamp",
            price: Some(1.0),
        };
        flat.write(lamp).await.unwrap();

        // Without a header to write, the csv crate would flatten `tags`.
        let nested = Nested {
            name: "Chair",
            tags: vec!["new", "old"],
        };
        let error = items.write(nested).await.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Dataset);
        let tuples = CsvDataset::<(&str, u32)>::from_sink(items.sink.clone());
        assert!(tuples.write(("Desk", 2)).await.is_err());

        let contents = tokio::fs::read_to_string(&path).await.unwrap();
        tokio::fs::remove_file(&path).await.unwrap();
        assert_eq!(contents, "name,price\nLamp,1.0\n");
    }

    #[tokio::test]
    async fn appending_checks_the_existing_header() {
        let path = temp_path("csv-header");
        tokio::fs::write(&path, "title,words\nHome,3\n")
            .await
            .unwrap();
        let products = CsvDataset::<Product>::append(&path).await.unwrap();
        let lamp = Product {
            name: "Lamp",
            price: None,
        };
        assert!(products.write(lamp).await.is_err());

        // Tuples have no header, so they must match its number of fields.
        drop(products);
        let pages = CsvDataset::<(&str, u32)>::append(&path).await.unwrap();
        pages.write(("About", 5)).await.unwrap();
        let wider = CsvDataset::<(&str, u32, u32)>::from_sink(pages.sink.clone());
        assert!(wider.write(("About", 5, 1)).await.is_err());

        let contents = tokio::fs::read_to_string(&path).await.unwrap();
        tokio::fs::remove_file(&path).await.unwrap();
        assert_eq!(contents, "title,words\nHome,3\nAbout,5\n");
    }
}
