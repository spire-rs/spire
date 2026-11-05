use std::error::Error as StdError;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use redb::{
    Database, Durability, ReadTransaction, ReadableDatabase, ReadableTable, ReadableTableMetadata,
    TableDefinition, TableError, WriteTransaction,
};
use spire_core::{Error, ErrorKind, Result};

use crate::codec;
use crate::dataset::RedbDataset;
use crate::frontier::RedbFrontier;
use crate::seen::RedbSeen;
use crate::validator::RedbValidator;

/// Queued seeds, by negated priority then insertion order.
pub(crate) const QUEUE: TableDefinition<(i64, u64), &[u8]> = TableDefinition::new("spire/queue/v1");
/// Popped seeds not yet reported, by fingerprint.
pub(crate) const IN_FLIGHT: TableDefinition<u64, &[u8]> =
    TableDefinition::new("spire/in_flight/v1");
/// Seeds to crawl again, one per fingerprint: when they are due, in
/// milliseconds since the Unix epoch, then the encoded seed.
pub(crate) const SCHEDULED: TableDefinition<u64, &[u8]> =
    TableDefinition::new("spire/scheduled/v1");
/// The seeds of [`SCHEDULED`] by when they are due, then fingerprint.
pub(crate) const SCHEDULE: TableDefinition<(u64, u64), ()> =
    TableDefinition::new("spire/schedule/v1");
/// Seeds waiting for a retry, by when they are due, in milliseconds since the
/// Unix epoch, then a sequence, so several of one page can wait.
pub(crate) const RETRIES: TableDefinition<(u64, u64), &[u8]> =
    TableDefinition::new("spire/retries/v1");
/// Versions of crawled pages, by their request's fingerprint, see
/// [`RedbValidator`].
pub(crate) const PAGE_VERSIONS: TableDefinition<u64, &[u8]> =
    TableDefinition::new("spire/page_versions/v1");
/// Fingerprints seen by [`RedbSeen`].
pub(crate) const SEEN: TableDefinition<u64, ()> = TableDefinition::new("spire/seen/v1");
/// Counters, such as the next insertion sequence.
pub(crate) const META: TableDefinition<&str, u64> = TableDefinition::new("spire/meta/v1");

/// The key of the next insertion sequence in [`META`].
pub(crate) const NEXT_SEQUENCE: &str = "next_sequence";
/// The key of the latest time the frontier saw in [`META`], in milliseconds
/// since the Unix epoch.
pub(crate) const LAST_SEEN: &str = "last_seen";

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
/// Small extensions to the crawl, such as a cache, can keep namespaced tables
/// in this database through [`read`](Self::read) and [`write`](Self::write).
/// Keep unrelated application tables, such as a search index, in a database
/// of their own: redb locks its file and lets one transaction write at a
/// time, so tables in the same file wait on the frontier's every commit, and
/// it on theirs.
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

/// Converts between stored due times, in milliseconds since the Unix epoch, and
/// the instants the frontier works with.
///
/// Reads the wall clock once, on opening, so due times carry over a restart,
/// and is monotonic from there, so a clock change does not move them while the
/// store is open. It starts no earlier than the latest time the store saw, so
/// a wall clock moved back between runs does not delay them either.
pub(crate) struct Clock {
    opened_at: u64,
    opened: tokio::time::Instant,
}

impl Clock {
    fn new(last_seen: u64) -> Self {
        let since = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let wall = u64::try_from(since.as_millis()).unwrap_or(u64::MAX);
        Self {
            opened_at: wall.max(last_seen),
            opened: tokio::time::Instant::now(),
        }
    }

    pub(crate) fn now(&self) -> u64 {
        self.millis_at(tokio::time::Instant::now())
    }

    pub(crate) fn millis_at(&self, at: tokio::time::Instant) -> u64 {
        let elapsed = at.saturating_duration_since(self.opened).as_millis();
        self.opened_at
            .saturating_add(u64::try_from(elapsed).unwrap_or(u64::MAX))
    }

    pub(crate) fn instant_at(&self, millis: u64) -> tokio::time::Instant {
        let wait = millis.saturating_sub(self.now());
        tokio::time::Instant::now() + Duration::from_millis(wait)
    }
}

struct Inner {
    db: Database,
    path: PathBuf,
    clock: Clock,
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
    /// Returns a builder with the default options, the same as
    /// [`RedbStore::builder`].
    pub fn new() -> Self {
        Self::default()
    }

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
            let (recovered, last_seen) = recover(&db).map_err(storage_error)?;
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
                clock: Clock::new(last_seen),
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
/// Returns how many it moved, and the latest time the frontier saw.
fn recover(db: &Database) -> Result<(usize, u64), redb::Error> {
    let txn = db.begin_write()?;
    let mut recovered = 0;
    let last_seen;
    {
        let mut queue = txn.open_table(QUEUE)?;
        let mut in_flight = txn.open_table(IN_FLIGHT)?;
        let mut meta = txn.open_table(META)?;
        txn.open_table(SEEN)?;
        txn.open_table(RETRIES)?;

        last_seen = meta.get(LAST_SEEN)?.map_or(0, |guard| guard.value());
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
    Ok((recovered, last_seen))
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

    /// Returns the page versions stored in this database, for conditional
    /// requests with `Revalidate`.
    #[must_use]
    pub fn validator(&self) -> RedbValidator {
        RedbValidator::new(self.clone())
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
    /// commits it using this store's durability policy. Returning an error
    /// rolls back what the callback wrote. Use a namespaced table name and
    /// leave the store's built-in tables untouched.
    ///
    /// In the callback, `?` works on redb's errors and on spire's. To reject
    /// the transaction for a reason of the extension's own, return a
    /// [`spire_core::Error`] converted with `into()`: it comes back as is.
    ///
    /// # Errors
    ///
    /// Returns the callback's error, or a [`Frontier`](ErrorKind::Frontier)
    /// error if the transaction or commit fails.
    pub async fn write<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&WriteTransaction) -> Result<T, RedbTxnError> + Send + 'static,
    {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut txn = inner.db.begin_write().map_err(storage_error)?;
            let durability = inner.durability();
            let syncs = matches!(durability, Durability::Immediate);
            txn.set_durability(durability).map_err(storage_error)?;
            // An error drops the transaction, which rolls it back.
            let value = f(&txn)?;
            txn.commit().map_err(storage_error)?;
            if syncs {
                *inner.last_sync() = Instant::now();
            }
            Ok(value)
        })
        .await
        .map_err(join_error)?
    }

    /// Runs an extension's read transaction on the calling thread, where it
    /// blocks: async code should move it to a blocking pool, such as with
    /// [`spawn_blocking`](tokio::task::spawn_blocking).
    ///
    /// In the callback, `?` works on redb's errors and on spire's.
    ///
    /// # Errors
    ///
    /// Returns the callback's error, or a [`Frontier`](ErrorKind::Frontier)
    /// error if the transaction cannot be opened.
    pub fn read<T>(
        &self,
        f: impl FnOnce(&ReadTransaction) -> Result<T, RedbTxnError>,
    ) -> Result<T> {
        let txn = self.inner.db.begin_read().map_err(storage_error)?;
        Ok(f(&txn)?)
    }

    /// Runs `f` on the blocking pool, for reads made from async code.
    pub(crate) async fn blocking<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&Self) -> Result<T> + Send + 'static,
    {
        let store = self.clone();
        tokio::task::spawn_blocking(move || f(&store))
            .await
            .map_err(join_error)?
    }

    pub(crate) fn clock(&self) -> &Clock {
        &self.inner.clock
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
    /// Commits durably always, or once per sync interval. The interval runs
    /// from the last durable commit, so a transaction that does not commit
    /// leaves the next one to sync.
    fn durability(&self) -> Durability {
        let Some(interval) = self.sync_interval else {
            return Durability::Immediate;
        };

        if self.last_sync().elapsed() >= interval {
            Durability::Immediate
        } else {
            Durability::None
        }
    }

    fn last_sync(&self) -> MutexGuard<'_, Instant> {
        self.last_sync
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
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

/// The error of a [`RedbStore::read`] or [`RedbStore::write`] callback: a
/// [`spire_core::Error`], or one of redb's errors, which `?` converts into a
/// [`Frontier`](ErrorKind::Frontier) error.
#[derive(Debug)]
pub struct RedbTxnError(Error);

impl fmt::Display for RedbTxnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl StdError for RedbTxnError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.0.source()
    }
}

impl From<Error> for RedbTxnError {
    fn from(error: Error) -> Self {
        Self(error)
    }
}

impl From<RedbTxnError> for Error {
    fn from(error: RedbTxnError) -> Self {
        error.0
    }
}

macro_rules! txn_error_from_redb {
    ($($error:ty),* $(,)?) => {
        $(
            impl From<$error> for RedbTxnError {
                fn from(error: $error) -> Self {
                    Self(storage_error(error))
                }
            }
        )*
    };
}

txn_error_from_redb!(
    redb::Error,
    redb::StorageError,
    redb::TableError,
    redb::SavepointError,
    redb::CursorError,
);

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

#[cfg(test)]
mod tests {
    use redb::TableHandle;
    use spire_core::dataset::Dataset;
    use spire_core::worker::middleware::{PageVersion, Validator};

    use super::*;
    use crate::frontier::tests::{TempPath, seed};

    const VISITS: TableDefinition<&str, u64> = TableDefinition::new("test/visits/v1");

    fn visits(store: &RedbStore) -> Option<u64> {
        store
            .read(|txn| {
                let table = match txn.open_table(VISITS) {
                    Ok(table) => table,
                    Err(TableError::TableDoesNotExist(_)) => return Ok(None),
                    Err(error) => return Err(error.into()),
                };
                Ok(table.get("example.com")?.map(|visits| visits.value()))
            })
            .unwrap()
    }

    #[tokio::test]
    async fn every_table_is_namespaced() {
        let path = TempPath::new();
        let store = RedbStore::open(&path.0).await.unwrap();
        store.dataset::<u32>("products").write(1).await.unwrap();
        let fingerprint = seed("a").fingerprint();
        let version = PageVersion::default();
        store.validator().set(fingerprint, version).await.unwrap();

        let names: Vec<String> = store
            .read(|txn| {
                let tables = txn.list_tables()?;
                Ok(tables.map(|table| table.name().to_owned()).collect())
            })
            .unwrap();
        assert!(names.contains(&"spire/dataset/products/v1".to_owned()));
        assert!(
            names.iter().all(|name| name.starts_with("spire/")),
            "{names:?}"
        );
    }

    #[tokio::test]
    async fn write_commits_on_ok_and_rolls_back_on_error() {
        let path = TempPath::new();
        let store = RedbStore::open(&path.0).await.unwrap();

        store
            .write(|txn| {
                txn.open_table(VISITS)?.insert("example.com", 1)?;
                Ok(())
            })
            .await
            .unwrap();
        assert_eq!(visits(&store), Some(1));

        let rejected: Result<()> = store
            .write(|txn| {
                txn.open_table(VISITS)?.insert("example.com", 2)?;
                Err(Error::invalid("too many visits").into())
            })
            .await;
        let rejected = rejected.unwrap_err();
        assert_eq!(rejected.kind(), ErrorKind::Invalid);
        assert_eq!(rejected.message(), "too many visits");
        assert_eq!(visits(&store), Some(1));
    }

    #[tokio::test]
    async fn a_storage_error_in_a_write_is_a_frontier_error() {
        let path = TempPath::new();
        let store = RedbStore::open(&path.0).await.unwrap();

        let failed: Result<()> = store
            .write(|txn| {
                txn.open_table(VISITS)?.insert("example.com", 1)?;
                Err(redb::Error::Io(std::io::Error::other("disk")).into())
            })
            .await;
        assert_eq!(failed.unwrap_err().kind(), ErrorKind::Frontier);
        assert_eq!(visits(&store), None);
    }

    #[tokio::test]
    async fn a_read_callback_error_comes_back_intact() {
        let path = TempPath::new();
        let store = RedbStore::open(&path.0).await.unwrap();

        let failed: Result<()> = store.read(|_| Err(Error::invalid("bad value").into()));
        let failed = failed.unwrap_err();
        assert_eq!(failed.kind(), ErrorKind::Invalid);
        assert_eq!(failed.message(), "bad value");
    }

    #[tokio::test]
    async fn only_a_committed_write_counts_as_a_sync() {
        let path = TempPath::new();
        let store = RedbStore::builder()
            .sync_interval(Duration::ZERO)
            .open(&path.0)
            .await
            .unwrap();
        let last_sync = || *store.inner.last_sync.lock().unwrap();
        let before = last_sync();

        let rejected: Result<()> = store
            .write(|_| Err(Error::invalid("rejected").into()))
            .await;
        assert!(rejected.is_err());
        assert_eq!(last_sync(), before);

        store.write(|_| Ok(())).await.unwrap();
        assert!(last_sync() > before);
    }
}
