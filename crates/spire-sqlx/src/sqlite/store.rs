use std::path::Path;
use std::sync::{Arc, Weak};
use std::time::Duration;

use spire_core::{Error, ErrorKind, Result};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use tokio::sync::Mutex;

use super::frontier::{Inner, SqliteFrontier};
use super::{SqliteDataset, SqliteSeen, SqliteValidator};

/// Persistent crawl storage in one SQLite database.
///
/// [`open`](Self::open) and [`builder`](Self::builder) create and configure
/// the database. [`new`](Self::new) uses a connection pool you configured
/// yourself, whose options the store never changes. Clones share the pool;
/// dropping a store does not explicitly close it, see [`close`](Self::close).
///
/// # Examples
///
/// ```no_run
/// use spire_sqlx::sqlite::SqliteStore;
///
/// # async fn example() -> spire_core::Result<()> {
/// let store = SqliteStore::open("crawl.sqlite").await?;
/// let titles = store.dataset::<String>("titles");
/// # store.close().await;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct SqliteStore {
    pool: SqlitePool,
    frontier: Arc<Mutex<Weak<Inner>>>,
}

/// Opens a [`SqliteStore`] with options, see [`SqliteStore::builder`].
#[derive(Debug, Clone)]
#[must_use]
pub struct SqliteStoreBuilder {
    sync_always: bool,
    busy_timeout: Duration,
    max_connections: u32,
}

impl Default for SqliteStoreBuilder {
    fn default() -> Self {
        Self {
            sync_always: false,
            busy_timeout: Duration::from_secs(5),
            max_connections: 1,
        }
    }
}

impl SqliteStoreBuilder {
    /// Returns a builder with the default options, the same as
    /// [`SqliteStore::builder`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Syncs every commit to disk, so a power loss or system crash loses no
    /// committed work. Each change then waits for the disk, which caps
    /// throughput.
    ///
    /// By default, commits survive an application crash, while a power loss
    /// or system crash may roll back the latest ones to an earlier consistent
    /// state.
    pub const fn sync_always(mut self) -> Self {
        self.sync_always = true;
        self
    }

    /// Waits up to `timeout` for another connection's write to finish before
    /// failing, five seconds by default.
    pub const fn busy_timeout(mut self, timeout: Duration) -> Self {
        self.busy_timeout = timeout;
        self
    }

    /// Opens at most `limit` connections, one by default and at least one.
    ///
    /// SQLite allows one writer at a time, so more connections only help
    /// concurrent reads.
    pub const fn max_connections(mut self, limit: u32) -> Self {
        self.max_connections = limit;
        self
    }

    /// Opens the database at `path`, creating it if needed, and applies
    /// Spire's schema migrations, see [`SqliteStore::initialize`].
    ///
    /// The database uses write-ahead logging. SQLite saves this in the file,
    /// so opening an existing database converts it, and it keeps `-wal` and
    /// `-shm` files beside it.
    ///
    /// # Errors
    ///
    /// Returns a [`Frontier`](ErrorKind::Frontier) error if the database
    /// cannot be opened, or the errors of [`SqliteStore::initialize`].
    pub async fn open(self, path: impl AsRef<Path>) -> Result<SqliteStore> {
        let path = path.as_ref();
        let synchronous = if self.sync_always {
            SqliteSynchronous::Full
        } else {
            SqliteSynchronous::Normal
        };
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(synchronous)
            .busy_timeout(self.busy_timeout);
        let pool = SqlitePoolOptions::new()
            .max_connections(self.max_connections.max(1))
            .connect_with(options)
            .await
            .map_err(|error| {
                Error::new(ErrorKind::Frontier)
                    .with_message(format!("failed to open `{}`", path.display()))
                    .with_source(error)
            })?;
        let store = SqliteStore::new(pool);
        if let Err(error) = store.initialize().await {
            store.close().await;
            return Err(error);
        }
        Ok(store)
    }
}

impl SqliteStore {
    /// Returns a builder to open a store with options.
    pub fn builder() -> SqliteStoreBuilder {
        SqliteStoreBuilder::default()
    }

    /// Opens the database at `path`, see [`SqliteStoreBuilder::open`].
    ///
    /// # Errors
    ///
    /// Returns a [`Frontier`](ErrorKind::Frontier) error if the database
    /// cannot be opened, or the errors of [`initialize`](Self::initialize).
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::builder().open(path).await
    }

    /// Uses an existing pool without creating or modifying any tables.
    ///
    /// This performs no SQL: call [`initialize`](Self::initialize) once before
    /// using storage services. Pool options, journal mode, synchronous
    /// settings, busy timeout and connection lifetime remain yours.
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            frontier: Arc::new(Mutex::new(Weak::new())),
        }
    }

    /// Applies Spire's versioned schema migrations atomically.
    ///
    /// This is explicit schema initialization, not an application migration
    /// runner. The `spire_dataset_*`, `spire_seen`, `spire_page_versions`,
    /// `spire_frontier_*` and `spire_frontier_scheduled*` names are reserved for this crate.
    /// Application tables are not altered or repurposed.
    /// Applications control when this runs and must grant schema permissions.
    /// Migration history is isolated in `_spire_sqlx_migrations`; the
    /// application's `_sqlx_migrations` table is untouched. Repeated calls are
    /// safe. Applied checksums and versions are validated before any upgrades.
    ///
    /// # Errors
    /// Returns a dataset error on SQL, schema or lock failures, including
    /// untracked reserved tables, changed migration checksums or unknown versions.
    pub async fn initialize(&self) -> Result<()> {
        super::migration::run(&self.pool).await
    }

    /// Opens a named JSON dataset without creating a table per name.
    ///
    /// Names are bound SQL values, so punctuation is safe. Handles with the
    /// same name share records, even when opened through another pool.
    /// Use the same record type for every handle to a name.
    #[must_use]
    pub fn dataset<T>(&self, name: impl Into<String>) -> SqliteDataset<T> {
        SqliteDataset::new(self.pool.clone(), name.into())
    }

    /// Acquires the file's exclusive frontier and recovers abandoned requests.
    ///
    /// Calls through clones of this store share one frontier. Other stores or
    /// processes are refused while any handle owns the sidecar lock. This
    /// requires a local file-backed database; datasets and other services may
    /// still share its pool through independent stores.
    ///
    /// # Errors
    /// Returns a frontier error if the schema is missing, the database is
    /// in-memory, the lock is held, or storage or file locking fails.
    pub async fn frontier(&self) -> Result<SqliteFrontier> {
        let mut frontier = self.frontier.lock().await;
        if let Some(inner) = frontier.upgrade() {
            return Ok(SqliteFrontier { inner });
        }
        let opened = SqliteFrontier::open(self.pool.clone()).await?;
        *frontier = Arc::downgrade(&opened.inner);
        Ok(opened)
    }

    /// Returns the persistent exact seen set sharing this store.
    #[must_use]
    pub fn seen(&self) -> SqliteSeen {
        SqliteSeen::new(self.pool.clone())
    }

    /// Returns a persistent validator sharing this store.
    #[must_use]
    pub fn validator(&self) -> SqliteValidator {
        SqliteValidator::new(self.pool.clone())
    }

    /// Returns the connection pool, for your own queries.
    #[must_use]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Closes the pool, waiting for connections in use to return.
    ///
    /// This closes it for every clone of this store, and for a pool passed to
    /// [`new`](Self::new), for everything else using it. Later storage
    /// operations fail. A frontier keeps its file lock until its last handle
    /// is dropped.
    pub async fn close(&self) {
        self.pool.close().await;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use spire_core::dataset::Dataset;
    use spire_core::frontier::seed::Seed;
    use spire_core::frontier::seen::SeenSet;
    use spire_core::frontier::{Frontier, NextSeed, Report};
    use spire_core::worker::middleware::{PageVersion, Validator};
    use tempfile::TempDir;

    use super::*;

    /// Reads a store's journal mode, `synchronous` level and busy timeout.
    async fn settings(store: &SqliteStore) -> (String, i64, i64) {
        let pool = store.pool();
        (
            sqlx::query_scalar("PRAGMA journal_mode")
                .fetch_one(pool)
                .await
                .unwrap(),
            sqlx::query_scalar("PRAGMA synchronous")
                .fetch_one(pool)
                .await
                .unwrap(),
            sqlx::query_scalar("PRAGMA busy_timeout")
                .fetch_one(pool)
                .await
                .unwrap(),
        )
    }

    #[tokio::test]
    async fn open_creates_the_database_with_the_default_settings() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("crawl.sqlite");
        let store = SqliteStore::open(&path).await.unwrap();
        assert!(path.exists());
        assert_eq!(settings(&store).await, ("wal".to_owned(), 1, 5000));
        assert_eq!(store.pool().options().get_max_connections(), 1);
        // The schema is ready without calling `initialize`.
        let titles = store.dataset::<String>("titles");
        titles.write("Example Domain".to_owned()).await.unwrap();
        assert_eq!(titles.len().await.unwrap(), 1);
        store.close().await;
    }

    #[tokio::test]
    async fn builder_options_reach_the_connections() {
        let directory = TempDir::new().unwrap();
        let store = SqliteStore::builder()
            .sync_always()
            .busy_timeout(Duration::from_secs(7))
            .max_connections(4)
            .open(directory.path().join("crawl.sqlite"))
            .await
            .unwrap();
        assert_eq!(settings(&store).await, ("wal".to_owned(), 2, 7000));
        assert_eq!(store.pool().options().get_max_connections(), 4);
        store.close().await;
    }

    #[tokio::test]
    async fn zero_connections_is_raised_to_one() {
        let directory = TempDir::new().unwrap();
        let store = SqliteStore::builder()
            .max_connections(0)
            .open(directory.path().join("crawl.sqlite"))
            .await
            .unwrap();
        assert_eq!(store.pool().options().get_max_connections(), 1);
        store.close().await;
    }

    #[tokio::test]
    async fn reopening_keeps_records_and_recovers_the_frontier() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("crawl.sqlite");
        let store = SqliteStore::open(&path).await.unwrap();
        store
            .dataset::<String>("titles")
            .write("kept".to_owned())
            .await
            .unwrap();
        let frontier = store.frontier().await.unwrap();
        frontier
            .push(Seed::get("https://example.com/").unwrap())
            .await
            .unwrap();
        // Abandoned in flight, as in a crash.
        assert!(matches!(frontier.pop().await.unwrap(), NextSeed::Ready(_)));
        drop(frontier);
        store.close().await;

        let reopened = SqliteStore::open(&path).await.unwrap();
        let titles = reopened.dataset::<String>("titles");
        assert_eq!(titles.read().await.unwrap().as_deref(), Some("kept"));
        let frontier = reopened.frontier().await.unwrap();
        assert!(matches!(frontier.pop().await.unwrap(), NextSeed::Ready(_)));
        drop(frontier);
        reopened.close().await;
    }

    #[tokio::test]
    async fn open_converts_an_existing_database_and_keeps_its_tables() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("app.sqlite");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true)
                    .journal_mode(SqliteJournalMode::Delete),
            )
            .await
            .unwrap();
        sqlx::raw_sql(
            "CREATE TABLE app_items (value TEXT); INSERT INTO app_items VALUES ('retained');",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;

        let store = SqliteStore::open(&path).await.unwrap();
        assert_eq!(settings(&store).await.0, "wal");
        let value: String = sqlx::query_scalar("SELECT value FROM app_items")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(value, "retained");
        store.close().await;
    }

    #[tokio::test]
    async fn open_reports_a_database_it_cannot_create() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("missing/crawl.sqlite");
        let error = SqliteStore::open(&path).await.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Frontier);
        assert!(error.to_string().contains("failed to open"));
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn open_reports_a_schema_it_cannot_apply() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("crawl.sqlite");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::query("CREATE TABLE spire_dataset_items (original TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
        let error = SqliteStore::open(&path).await.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Dataset);
    }

    #[tokio::test]
    async fn close_closes_the_pool_for_every_clone() {
        let directory = TempDir::new().unwrap();
        let store = SqliteStore::open(directory.path().join("crawl.sqlite"))
            .await
            .unwrap();
        let clone = store.clone();
        store.close().await;
        assert!(clone.pool().is_closed());
        assert!(
            clone
                .dataset::<String>("titles")
                .write("late".to_owned())
                .await
                .is_err()
        );
    }

    /// SQLite refuses a write that upgrades a read transaction while another
    /// connection writes, without waiting for the busy timeout, so every
    /// service has to work with several connections.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn services_work_concurrently_over_several_connections() {
        const SEEDS: u32 = 64;
        let directory = TempDir::new().unwrap();
        let store = SqliteStore::builder()
            .max_connections(4)
            .open(directory.path().join("crawl.sqlite"))
            .await
            .unwrap();
        let frontier = store.frontier().await.unwrap();

        let mut writers = Vec::new();
        for index in 0..SEEDS {
            let store = store.clone();
            writers.push(tokio::spawn(async move {
                let seed = Seed::get(format!("https://example.com/{index}")).unwrap();
                let key = seed.fingerprint();
                store.dataset::<u32>("items").write(index).await.unwrap();
                assert!(store.seen().insert(key).await.unwrap());
                store
                    .validator()
                    .set(key, PageVersion::default())
                    .await
                    .unwrap();
                store.frontier().await.unwrap().push(seed).await.unwrap();
            }));
        }
        for writer in writers {
            writer.await.unwrap();
        }

        let mut consumers = Vec::new();
        for _ in 0..SEEDS {
            let store = store.clone();
            consumers.push(tokio::spawn(async move {
                let frontier = store.frontier().await.unwrap();
                let NextSeed::Ready(seed) = frontier.pop().await.unwrap() else {
                    panic!("expected a seed");
                };
                let path = seed.uri().path().to_owned();
                frontier.report(Report::Done(seed)).await.unwrap();
                assert!(
                    store
                        .dataset::<u32>("items")
                        .read()
                        .await
                        .unwrap()
                        .is_some()
                );
                path
            }));
        }
        let mut paths = HashSet::new();
        for consumer in consumers {
            assert!(paths.insert(consumer.await.unwrap()));
        }

        assert_eq!(paths.len(), SEEDS as usize);
        assert_eq!(store.seen().len().await.unwrap(), SEEDS as usize);
        assert_eq!(store.validator().len().await.unwrap(), SEEDS as usize);
        assert_eq!(frontier.in_flight().await.unwrap(), 0);
        assert!(store.dataset::<u32>("items").is_empty().await.unwrap());
        drop(frontier);
        store.close().await;
    }

    #[tokio::test]
    async fn initialization_preserves_application_schema_and_pool_settings() {
        let directory = TempDir::new().unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(directory.path().join("test.sqlite"))
                    .create_if_missing(true)
                    .journal_mode(SqliteJournalMode::Wal)
                    .synchronous(SqliteSynchronous::Full)
                    .busy_timeout(Duration::from_secs(7)),
            )
            .await
            .unwrap();
        sqlx::raw_sql(
            "CREATE TABLE app_items (value TEXT); INSERT INTO app_items VALUES ('retained');",
        )
        .execute(&pool)
        .await
        .unwrap();
        let store = SqliteStore::new(pool.clone());
        // Constructing the handle has no schema side effects.
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_schema WHERE name LIKE 'spire_%'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 0);
        store.initialize().await.unwrap();
        store.clone().initialize().await.unwrap();
        let value: String = sqlx::query_scalar("SELECT value FROM app_items")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(value, "retained");
        let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&pool)
            .await
            .unwrap();
        let synchronous: i64 = sqlx::query_scalar("PRAGMA synchronous")
            .fetch_one(&pool)
            .await
            .unwrap();
        let timeout: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!((mode.as_str(), synchronous, timeout), ("wal", 2, 7000));
        drop(store);
        assert!(!pool.is_closed());
        pool.close().await;
    }

    #[tokio::test]
    async fn incompatible_reserved_schema_is_not_rewritten() {
        let directory = TempDir::new().unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(directory.path().join("test.sqlite"))
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::query("CREATE TABLE spire_dataset_items (original TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        let store = SqliteStore::new(pool.clone());
        assert_eq!(
            store.initialize().await.unwrap_err().kind(),
            ErrorKind::Dataset
        );
        sqlx::query("INSERT INTO spire_dataset_items (original) VALUES ('untouched')")
            .execute(&pool)
            .await
            .unwrap();
        let value: String = sqlx::query_scalar("SELECT original FROM spire_dataset_items")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(value, "untouched");
        pool.close().await;
    }
    #[tokio::test]
    async fn existing_columns_without_fifo_primary_key_are_rejected() {
        let directory = TempDir::new().unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(directory.path().join("test.sqlite"))
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::query("CREATE TABLE spire_dataset_items (id INTEGER, dataset TEXT NOT NULL, payload TEXT NOT NULL)")
            .execute(&pool).await.unwrap();
        let store = SqliteStore::new(pool.clone());
        assert_eq!(
            store.initialize().await.unwrap_err().kind(),
            ErrorKind::Dataset
        );
        // Initialization must also roll back the newly created index.
        let indexes: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name = 'spire_dataset_order'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(indexes, 0);
        pool.close().await;
    }
}
