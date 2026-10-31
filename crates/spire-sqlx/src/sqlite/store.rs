use std::sync::{Arc, Weak};

use spire_core::Result;
use sqlx::SqlitePool;
use tokio::sync::Mutex;

use super::frontier::{Inner, SqliteFrontier};
use super::{SqliteDataset, SqliteSeen, SqliteValidators};

/// Persistent crawl storage sharing a caller-owned SQLite connection pool.
///
/// Construction performs no SQL. Call [`initialize`](Self::initialize) once
/// before using storage services. Pool options, journal mode, synchronous
/// settings, busy timeout and connection lifetime remain caller-controlled.
/// Clones share the pool; dropping a store does not explicitly close it.
#[derive(Debug, Clone)]
pub struct SqliteStore {
    pool: SqlitePool,
    frontier: Arc<Mutex<Weak<Inner>>>,
}

impl SqliteStore {
    /// Uses an existing pool without creating or modifying any tables.
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
    /// runner. The `spire_dataset_*`, `spire_seen`, `spire_validators`,
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

    /// Returns persistent HTTP validators sharing this store.
    #[must_use]
    pub fn validators(&self) -> SqliteValidators {
        SqliteValidators::new(self.pool.clone())
    }

    /// Returns the caller-owned pool for application queries and shutdown.
    #[must_use]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use spire_core::ErrorKind;
    use sqlx::sqlite::{
        SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous,
    };
    use tempfile::TempDir;

    use super::*;

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
