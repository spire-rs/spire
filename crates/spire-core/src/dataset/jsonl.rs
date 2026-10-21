use std::fmt;
use std::future::{Future, ready};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::Serialize;
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use crate::dataset::Dataset;
use crate::{Error, ErrorKind, Result};

/// A write-only [`Dataset`] that appends every item to a file as one line of
/// JSON ([JSON Lines](https://jsonlines.org)).
///
/// Each item is written and flushed as it is stored, under a lock, so lines
/// never interleave. A flush hands the line to the operating system without
/// syncing it to disk: items survive a crash of the crawl, not of the
/// machine. Reading returns nothing; read the file instead. Clones share
/// the file.
///
/// Writes are not part of the frontier's state, so after a crash a request
/// whose items were written may be crawled again, writing them twice: dedupe
/// by a key such as the URL when reading the file.
///
/// ```no_run
/// use spire_core::dataset::JsonlDataset;
///
/// #[derive(serde::Serialize)]
/// struct Page {
///     url: String,
///     title: String,
/// }
///
/// # async fn example() -> spire_core::Result<()> {
/// let pages = JsonlDataset::<Page>::create("pages.jsonl").await?;
/// # Ok(())
/// # }
/// ```
pub struct JsonlDataset<T> {
    inner: Arc<Inner>,
    marker: PhantomData<fn(T)>,
}

struct Inner {
    file: Mutex<File>,
    path: PathBuf,
    written: AtomicUsize,
}

impl<T> JsonlDataset<T> {
    /// Creates the file, truncating it if it exists.
    ///
    /// # Errors
    ///
    /// Returns a [`Dataset`](ErrorKind::Dataset) error if the file cannot be
    /// created.
    pub async fn create(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let file = File::create(path)
            .await
            .map_err(|error| open_error(path, error))?;
        Ok(Self::from_file(file, path))
    }

    /// Opens the file to append to it, creating it if it does not exist,
    /// e.g. to resume a crawl.
    ///
    /// # Errors
    ///
    /// Returns a [`Dataset`](ErrorKind::Dataset) error if the file cannot be
    /// opened.
    pub async fn append(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await
            .map_err(|error| open_error(path, error))?;
        Ok(Self::from_file(file, path))
    }

    fn from_file(file: File, path: &Path) -> Self {
        let inner = Inner {
            file: Mutex::new(file),
            path: path.to_owned(),
            written: AtomicUsize::new(0),
        };

        Self {
            inner: Arc::new(inner),
            marker: PhantomData,
        }
    }

    /// Returns the path of the file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.inner.path
    }
}

fn open_error(path: &Path, error: std::io::Error) -> Error {
    let message = format!("failed to open `{}`", path.display());
    Error::new(ErrorKind::Dataset)
        .with_message(message)
        .with_source(error)
}

impl<T> Clone for JsonlDataset<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            marker: PhantomData,
        }
    }
}

impl<T> fmt::Debug for JsonlDataset<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JsonlDataset")
            .field("path", &self.inner.path)
            .field("written", &self.inner.written.load(Ordering::Relaxed))
            .finish()
    }
}

impl<T> Dataset<T> for JsonlDataset<T>
where
    T: Serialize + Send + 'static,
{
    async fn write(&self, item: T) -> Result<()> {
        let mut line = serde_json::to_vec(&item).map_err(|error| {
            Error::new(ErrorKind::Dataset)
                .with_message("failed to serialize an item")
                .with_source(error)
        })?;
        line.push(b'\n');

        let write_error = |error: std::io::Error| {
            let message = format!("failed to write to `{}`", self.inner.path.display());
            Error::new(ErrorKind::Dataset)
                .with_message(message)
                .with_source(error)
        };
        let mut file = self.inner.file.lock().await;
        file.write_all(&line).await.map_err(write_error)?;
        file.flush().await.map_err(write_error)?;

        self.inner.written.fetch_add(1, Ordering::Relaxed);
        Ok(())
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
    use super::*;

    #[derive(Serialize)]
    struct Page {
        url: &'static str,
        words: u32,
    }

    fn temp_path(name: &str) -> PathBuf {
        let unique = format!("spire-{name}-{}.jsonl", std::process::id());
        std::env::temp_dir().join(unique)
    }

    #[tokio::test]
    async fn writes_one_line_per_item_and_appends() {
        let path = temp_path("jsonl");
        let pages = JsonlDataset::<Page>::create(&path).await.unwrap();
        pages
            .write(Page {
                url: "/a",
                words: 3,
            })
            .await
            .unwrap();
        pages
            .clone()
            .write(Page {
                url: "/b",
                words: 5,
            })
            .await
            .unwrap();
        assert!(pages.read().await.unwrap().is_none());

        let resumed = JsonlDataset::<Page>::append(&path).await.unwrap();
        resumed
            .write(Page {
                url: "/c",
                words: 1,
            })
            .await
            .unwrap();

        let contents = tokio::fs::read_to_string(&path).await.unwrap();
        tokio::fs::remove_file(&path).await.unwrap();
        assert_eq!(
            contents,
            "{\"url\":\"/a\",\"words\":3}\n{\"url\":\"/b\",\"words\":5}\n{\"url\":\"/c\",\"words\":1}\n"
        );
    }

    #[tokio::test]
    async fn reports_unopenable_paths() {
        let path = temp_path("missing-dir").join("nested/pages.jsonl");
        let error = JsonlDataset::<Page>::create(&path).await.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Dataset);
    }
}
