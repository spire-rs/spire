//! The file handling shared by the datasets that append to a file.

use std::fmt;
use std::fs::TryLockError;
use std::future::Future;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

use crate::{Error, ErrorKind, Result};

/// A file that items are appended to, with the encoder's state `S`.
///
/// The file is locked while any clone is alive, so a second sink on the
/// same file, in this process or another, fails to open instead of mixing
/// its writes in. Writes hold a lock, so items never interleave. A write
/// that fails partway is cut off, so the file never holds half an item, and
/// the encoder's state only moves on once its bytes are written.
pub(super) struct FileSink<S, T = File> {
    inner: Arc<Inner<S, T>>,
}

struct Inner<S, T> {
    open: Mutex<Open<S, T>>,
    path: PathBuf,
    written: AtomicUsize,
}

struct Open<S, T> {
    target: T,
    /// The length of the file after the last complete write.
    len: u64,
    state: S,
}

/// Where a [`FileSink`] writes: a file, or a stand-in in tests.
pub(super) trait Target: Send + 'static {
    /// Appends `bytes` and flushes them.
    fn append(&mut self, bytes: &[u8]) -> impl Future<Output = std::io::Result<()>> + Send;

    /// Cuts the target back to `len` bytes, the next append writing there.
    fn truncate(&mut self, len: u64) -> impl Future<Output = std::io::Result<()>> + Send;
}

impl Target for File {
    async fn append(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.write_all(bytes).await?;
        self.flush().await
    }

    async fn truncate(&mut self, len: u64) -> std::io::Result<()> {
        self.set_len(len).await?;
        self.seek(SeekFrom::Start(len)).await.map(drop)
    }
}

impl<S> FileSink<S> {
    /// Creates the file, emptying it if it exists.
    pub(super) async fn create(path: &Path, state: S) -> Result<Self> {
        let mut options = OpenOptions::new();
        options.write(true).create(true);
        // Emptied only once locked, so a file another sink is writing to is
        // left alone.
        let mut file = open_locked(path, &options).await?;
        file.truncate(0)
            .await
            .map_err(|error| open_error(path, error))?;
        Ok(Self::new(file, 0, path, state))
    }

    /// Opens the file to append to it, creating it if it does not exist.
    /// `state` gets the file's first line, empty if the file is.
    pub(super) async fn append(path: &Path, state: impl FnOnce(&[u8]) -> S) -> Result<Self> {
        let mut options = OpenOptions::new();
        options.read(true).append(true).create(true);
        let file = open_locked(path, &options).await?;

        let mut first = Vec::new();
        let mut reader = BufReader::new(file);
        reader
            .read_until(b'\n', &mut first)
            .await
            .map_err(|error| open_error(path, error))?;
        let file = reader.into_inner();
        let len = file
            .metadata()
            .await
            .map_err(|error| open_error(path, error))?
            .len();
        Ok(Self::new(file, len, path, state(&first)))
    }
}

/// Opens the file at `path` with `options`, and locks it.
async fn open_locked(path: &Path, options: &OpenOptions) -> Result<File> {
    let file = options
        .open(path)
        .await
        .map_err(|error| open_error(path, error))?;
    let file = file.into_std().await;
    match file.try_lock() {
        Ok(()) => Ok(File::from_std(file)),
        Err(TryLockError::WouldBlock) => {
            let message = format!("`{}` is already open by another dataset", path.display());
            Err(Error::new(ErrorKind::Dataset).with_message(message))
        }
        Err(TryLockError::Error(error)) => Err(open_error(path, error)),
    }
}

impl<S, T: Target> FileSink<S, T> {
    fn new(target: T, len: u64, path: &Path, state: S) -> Self {
        let open = Open { target, len, state };
        let inner = Inner {
            open: Mutex::new(open),
            path: path.to_owned(),
            written: AtomicUsize::new(0),
        };
        Self {
            inner: Arc::new(inner),
        }
    }

    /// Returns the path of the file.
    pub(super) fn path(&self) -> &Path {
        &self.inner.path
    }

    /// Returns the number of items written through this sink and its clones.
    pub(super) fn written(&self) -> usize {
        self.inner.written.load(Ordering::Relaxed)
    }

    /// Encodes an item with `encode`, which returns its bytes and the next
    /// state, then appends the bytes.
    pub(super) async fn write(
        &self,
        encode: impl FnOnce(&S) -> Result<(Vec<u8>, S)>,
    ) -> Result<()> {
        let mut open = self.inner.open.lock().await;
        let (bytes, state) = encode(&open.state)?;

        if let Err(error) = open.target.append(&bytes).await {
            // Cut off whatever part was written, so the next item starts on
            // a clean line. If that fails too, the original error is the one
            // worth reporting.
            let len = open.len;
            let _ = open.target.truncate(len).await;
            let message = format!("failed to write to `{}`", self.inner.path.display());
            return Err(Error::new(ErrorKind::Dataset)
                .with_message(message)
                .with_source(error));
        }

        open.len += bytes.len() as u64;
        open.state = state;
        self.inner.written.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

fn open_error(path: &Path, error: std::io::Error) -> Error {
    let message = format!("failed to open `{}`", path.display());
    Error::new(ErrorKind::Dataset)
        .with_message(message)
        .with_source(error)
}

impl<S, T> fmt::Debug for FileSink<S, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSink")
            .field("path", &self.inner.path)
            .field("written", &self.inner.written.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl<S, T> Clone for FileSink<S, T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A target that writes only half of the next `failures` appends, then
    /// fails them.
    #[derive(Default)]
    struct Flaky {
        bytes: Vec<u8>,
        failures: usize,
    }

    impl Target for Flaky {
        fn append(&mut self, bytes: &[u8]) -> impl Future<Output = std::io::Result<()>> + Send {
            let result = if self.failures == 0 {
                self.bytes.extend_from_slice(bytes);
                Ok(())
            } else {
                self.failures -= 1;
                self.bytes.extend_from_slice(&bytes[..bytes.len() / 2]);
                Err(std::io::Error::other("disk full"))
            };
            std::future::ready(result)
        }

        fn truncate(&mut self, len: u64) -> impl Future<Output = std::io::Result<()>> + Send {
            self.bytes.truncate(usize::try_from(len).unwrap());
            std::future::ready(Ok(()))
        }
    }

    /// Writes `line`, with a header the first time, as the CSV dataset does.
    async fn write(sink: &FileSink<bool, Flaky>, line: &str) -> Result<()> {
        sink.write(|header| {
            let mut bytes = if *header {
                Vec::new()
            } else {
                b"head\n".to_vec()
            };
            bytes.extend_from_slice(line.as_bytes());
            Ok((bytes, true))
        })
        .await
    }

    #[tokio::test]
    async fn failed_writes_leave_no_partial_item_and_no_state_change() {
        let flaky = Flaky {
            failures: 1,
            ..Flaky::default()
        };
        let sink = FileSink::new(flaky, 0, Path::new("test"), false);

        let error = write(&sink, "first\n").await.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Dataset);
        // The header is written again, since the failed write never was.
        write(&sink, "second\n").await.unwrap();
        write(&sink, "third\n").await.unwrap();

        let open = sink.inner.open.lock().await;
        assert_eq!(open.target.bytes, b"head\nsecond\nthird\n");
        assert_eq!(open.len, 18);
        assert_eq!(sink.written(), 2);
    }

    #[tokio::test]
    async fn a_file_opens_in_one_sink_at_a_time() {
        let path = std::env::temp_dir().join(format!("spire-lock-{}.txt", std::process::id()));
        let first = FileSink::create(&path, ()).await.unwrap();
        let clone = first.clone();

        let error = FileSink::append(&path, |_| ()).await.unwrap_err();
        assert!(error.to_string().contains("already open"), "{error}");
        assert!(FileSink::create(&path, ()).await.is_err());

        // The lock lasts until the last clone is dropped.
        drop(first);
        assert!(FileSink::append(&path, |_| ()).await.is_err());
        drop(clone);
        FileSink::append(&path, |_| ()).await.unwrap();
        tokio::fs::remove_file(&path).await.unwrap();
    }
}
