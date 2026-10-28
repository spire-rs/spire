use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use redb::{
    Database, Durability, ReadTransaction, ReadableDatabase, ReadableTable, ReadableTableMetadata,
    TableDefinition, TableError, WriteTransaction,
};
use spire_core::{Error, ErrorKind, Result};

use crate::codec;
use crate::dataset::RedbDataset;
use crate::frontier::RedbFrontier;
use crate::seen::RedbSeen;
use crate::validators::RedbValidators;

/// Queued seeds, by negated priority then insertion order.
pub(crate) const QUEUE: TableDefinition<(i64, u64), &[u8]> = TableDefinition::new("queue");
/// Popped seeds not yet reported, by fingerprint.
pub(crate) const IN_FLIGHT: TableDefinition<u64, &[u8]> = TableDefinition::new("in_flight");
/// Seeds to crawl again, one per fingerprint: when they are due, in
/// milliseconds since the Unix epoch, then the encoded seed.
pub(crate) const SCHEDULED: TableDefinition<u64, &[u8]> = TableDefinition::new("scheduled");
/// The seeds of [`SCHEDULED`] by when they are due, then fingerprint.
pub(crate) const SCHEDULE: TableDefinition<(u64, u64), ()> = TableDefinition::new("schedule");
/// Validators of crawled pages, by their request's fingerprint, see
/// [`RedbValidators`].
pub(crate) const VALIDATORS: TableDefinition<u64, &[u8]> = TableDefinition::new("validators");
/// Fingerprints seen by [`RedbSeen`].
pub(crate) const SEEN: TableDefinition<u64, ()> = TableDefinition::new("seen");
/// Counters, such as the next insertion sequence.
pub(crate) const META: TableDefinition<&str, u64> = TableDefinition::new("meta");

/// The key of the next insertion sequence in [`META`].
pub(crate) const NEXT_SEQUENCE: &str = "next_sequence";

/// A redb database holding a crawl's [`RedbFrontier`], [`RedbSeen`] and
/// [`RedbDataset`]s.
///
/// Every change is committed as it happens, so a crawl survives a crash:
/// reopening the store queues again the seeds that were in flight, which
/// are then crawled again. Clones share the database.
///
/// Commits reach the disk at most once a second by default. A crash rolls
/// the store back to its last synced commit, a consistent earlier state, so
/// it repeats at most a second of work and loses none: a link is never
/// committed after the page that found it. Use
/// [`RedbStoreBuilder::sync_always`] to sync every commit instead, at a
/// large cost in throughput.
///
/// The database is the crawl's own. Keep an application's own tables, such
/// as a search index, in a database of their own: redb locks its file and
/// lets one transaction write at a time, so tables in the same file would
/// wait on the frontier's every commit, and it on theirs.
///
/// ```no_run
/// use spire_core::frontier::FrontierExt;
/// use spire_redb::RedbStore;
///
/// # async fn example() -> spire_core::Result<()> {
/// let store = RedbStore::open("crawl.redb").await?;
/// let frontier = store.frontier().dedup_with(store.seen()).retry(3);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct RedbStore {
    inner: Arc<Inner>,
}

struct Inner {
    db: Database,
    path: PathBuf,
    sync_interval: Option<Duration>,
    last_sync: Mutex<Instant>,
}

/// Opens a [`RedbStore`] with options, see [`RedbStore::builder`].
#[derive(Debug, Clone)]
#[must_use]
pub struct RedbStoreBuilder {
    sync_interval: Option<Duration>,
}

impl Default for RedbStoreBuilder {
    fn default() -> Self {
        Self {
            sync_interval: Some(Duration::from_secs(1)),
        }
    }
}

impl RedbStoreBuilder {
    /// Syncs commits to disk at most every `interval`, one second by
    /// default. A crash repeats up to `interval` of work and loses none.
    /// Pending commits are synced when the store is dropped.
    pub const fn sync_interval(mut self, interval: Duration) -> Self {
        self.sync_interval = Some(interval);
        self
    }

    /// Syncs every commit to disk, so a crash repeats only the requests in
    /// flight. Each change then waits for the disk, which caps throughput.
    pub const fn sync_always(mut self) -> Self {
        self.sync_interval = None;
        self
    }

    /// Opens the database at `path`, creating it if needed, and queues again
    /// the seeds that were in flight when it was last closed.
    ///
    /// # Errors
    ///
    /// Returns a [`Frontier`](ErrorKind::Frontier) error if the database
    /// cannot be opened or recovered.
    pub async fn open(self, path: impl AsRef<Path>) -> Result<RedbStore> {
        let path = path.as_ref().to_owned();
        let sync_interval = self.sync_interval;
        tokio::task::spawn_blocking(move || {
            let db = Database::create(&path).map_err(|error| {
                let message = format!("failed to open `{}`", path.display());
                Error::new(ErrorKind::Frontier)
                    .with_message(message)
                    .with_source(error)
            })?;
            let recovered = recover(&db).map_err(storage_error)?;
            if recovered > 0 {
                tracing::info!(
                    target: crate::TRACING_TARGET,
                    path = %path.display(),
                    recovered,
                    "queued again the seeds in flight when the store was last closed",
                );
            }

            let inner = Inner {
                db,
                path,
                sync_interval,
                last_sync: Mutex::new(Instant::now()),
            };
            Ok(RedbStore {
                inner: Arc::new(inner),
            })
        })
        .await
        .map_err(join_error)?
    }
}

/// Creates every table, and moves the in-flight seeds back to the queue.
fn recover(db: &Database) -> Result<usize, redb::Error> {
    let txn = db.begin_write()?;
    let mut recovered = 0;
    {
        let mut queue = txn.open_table(QUEUE)?;
        let mut in_flight = txn.open_table(IN_FLIGHT)?;
        let mut meta = txn.open_table(META)?;
        txn.open_table(SEEN)?;

        let mut sequence = meta.get(NEXT_SEQUENCE)?.map_or(0, |guard| guard.value());
        while let Some((_, bytes)) = in_flight.pop_first()? {
            let bytes = bytes.value().to_vec();
            if let Some(priority) = codec::priority(&bytes) {
                queue.insert((-i64::from(priority), sequence), bytes.as_slice())?;
                sequence += 1;
                recovered += 1;
            }
        }
        meta.insert(NEXT_SEQUENCE, sequence)?;
    }
    txn.commit()?;
    Ok(recovered)
}

impl RedbStore {
    /// Returns a builder to open a store with options.
    pub fn builder() -> RedbStoreBuilder {
        RedbStoreBuilder::default()
    }

    /// Opens the database at `path`, see [`RedbStoreBuilder::open`].
    ///
    /// # Errors
    ///
    /// Returns a [`Frontier`](ErrorKind::Frontier) error if the database
    /// cannot be opened or recovered.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::builder().open(path).await
    }

    /// Returns the frontier stored in this database.
    #[must_use]
    pub fn frontier(&self) -> RedbFrontier {
        RedbFrontier::new(self.clone())
    }

    /// Returns the seen set stored in this database.
    #[must_use]
    pub fn seen(&self) -> RedbSeen {
        RedbSeen::new(self.clone())
    }

    /// Returns the validators stored in this database, for conditional
    /// requests with `Revalidate`.
    #[must_use]
    pub fn validators(&self) -> RedbValidators {
        RedbValidators::new(self.clone())
    }

    /// Returns the dataset stored in this database under `name`, created on
    /// its first write. Each name is a separate table: give each item type
    /// its own, and keep a name when renaming the type to keep its items.
    #[must_use]
    pub fn dataset<T>(&self, name: &str) -> RedbDataset<T> {
        RedbDataset::new(self.clone(), name)
    }

    /// Returns the path of the database.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.inner.path
    }

    /// Runs an extension's write transaction off the async runtime and
    /// commits it using this store's durability policy. Use a namespaced
    /// table name and leave the store's built-in tables untouched.
    ///
    /// # Errors
    ///
    /// Returns a [`Frontier`](ErrorKind::Frontier) error if the transaction,
    /// callback or commit fails. A failed callback is not committed.
    pub async fn write<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&WriteTransaction) -> Result<T, redb::Error> + Send + 'static,
    {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut txn = inner.db.begin_write().map_err(storage_error)?;
            txn.set_durability(inner.durability())
                .map_err(storage_error)?;
            let value = f(&txn).map_err(storage_error)?;
            txn.commit().map_err(storage_error)?;
            Ok(value)
        })
        .await
        .map_err(join_error)?
    }

    /// Runs an extension's read transaction on the calling thread. Async
    /// extensions can dispatch this to a blocking pool for large reads.
    ///
    /// # Errors
    ///
    /// Returns a [`Frontier`](ErrorKind::Frontier) error if the transaction
    /// cannot be opened or the callback fails.
    pub fn read<T>(&self, f: impl FnOnce(&ReadTransaction) -> Result<T, redb::Error>) -> Result<T> {
        let txn = self.inner.db.begin_read().map_err(storage_error)?;
        f(&txn).map_err(storage_error)
    }

    /// Returns the number of entries in `table`, zero if it was never
    /// created.
    pub(crate) fn count<K, V>(&self, table: TableDefinition<'_, K, V>) -> Result<usize>
    where
        K: redb::Key + 'static,
        V: redb::Value + 'static,
    {
        let len = self.read(|txn| match txn.open_table(table) {
            Ok(table) => Ok(table.len()?),
            Err(TableError::TableDoesNotExist(_)) => Ok(0),
            Err(error) => Err(error.into()),
        })?;
        Ok(usize::try_from(len).unwrap_or(usize::MAX))
    }
}

impl Inner {
    /// Commits durably always, or once per sync interval.
    fn durability(&self) -> Durability {
        let Some(interval) = self.sync_interval else {
            return Durability::Immediate;
        };

        let mut last_sync = self
            .last_sync
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if last_sync.elapsed() >= interval {
            *last_sync = Instant::now();
            Durability::Immediate
        } else {
            Durability::None
        }
    }
}

impl Drop for Inner {
    /// Writes changes still pending under a sync interval.
    fn drop(&mut self) {
        if self.sync_interval.is_none() {
            return;
        }
        let flush = || -> Result<(), redb::Error> {
            self.db.begin_write()?.commit()?;
            Ok(())
        };
        let flushed = flush();
        if let Err(error) = flushed {
            tracing::warn!(target: crate::TRACING_TARGET, %error, "failed to write pending changes");
        }
    }
}

impl fmt::Debug for RedbStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedbStore")
            .field("path", &self.inner.path)
            .field("sync_interval", &self.inner.sync_interval)
            .finish_non_exhaustive()
    }
}

/// Converts a storage failure into a frontier error.
pub(crate) fn storage_error(error: impl Into<redb::Error>) -> Error {
    Error::new(ErrorKind::Frontier)
        .with_message("redb storage failed")
        .with_source(error.into())
}

fn join_error(error: tokio::task::JoinError) -> Error {
    Error::new(ErrorKind::Frontier)
        .with_message("redb task failed")
        .with_source(error)
}
