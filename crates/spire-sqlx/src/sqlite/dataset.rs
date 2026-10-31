use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use serde::Serialize;
use serde::de::DeserializeOwned;
use spire_core::Result;
use spire_core::dataset::Dataset;
use sqlx::{Row, SqlitePool};

use crate::error::dataset_error;

/// A persistent FIFO dataset of JSON records in a [`super::SqliteStore`].
///
/// Writes append records and reads remove them. Every batch is one transaction;
/// failed SQL or decoding rolls it back. Concurrent consumers, including
/// separate processes, cannot successfully take the same record twice.
/// FIFO follows insertion IDs assigned under SQLite's single writer lock,
/// not the order concurrent tasks were started or returned to their callers.
/// Within a write batch, input order is preserved.
///
/// Destructive reads commit before returning records. Cancellation or a crash
/// after commit can lose delivery to the caller; this is not an acknowledged
/// queue. Dataset writes and frontier updates are independent transactions.
/// A page replayed after a crash can therefore write duplicate records.
pub struct SqliteDataset<T> {
    pool: SqlitePool,
    name: Arc<str>,
    marker: PhantomData<fn(T) -> T>,
}

impl<T> SqliteDataset<T> {
    pub(crate) fn new(pool: SqlitePool, name: String) -> Self {
        Self {
            pool,
            name: name.into(),
            marker: PhantomData,
        }
    }

    /// Counts currently stored items using an asynchronous query.
    ///
    /// # Errors
    /// Returns a dataset error if the query fails or the count exceeds `usize`.
    pub async fn len(&self) -> Result<usize> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM spire_dataset_items WHERE dataset = $1")
                .bind(&*self.name)
                .fetch_one(&self.pool)
                .await
                .map_err(dataset_error)?;
        usize::try_from(count).map_err(dataset_error)
    }

    /// Whether the dataset currently contains no records.
    ///
    /// # Errors
    /// Returns a dataset error if the query fails.
    pub async fn is_empty(&self) -> Result<bool> {
        let present: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM spire_dataset_items WHERE dataset = $1)",
        )
        .bind(&*self.name)
        .fetch_one(&self.pool)
        .await
        .map_err(dataset_error)?;
        Ok(!present)
    }
}

impl<T> Clone for SqliteDataset<T> {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            name: self.name.clone(),
            marker: PhantomData,
        }
    }
}

impl<T> fmt::Debug for SqliteDataset<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SqliteDataset")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl<T: Serialize + DeserializeOwned + Send + 'static> Dataset<T> for SqliteDataset<T> {
    async fn write(&self, item: T) -> Result<()> {
        self.write_batch(vec![item]).await
    }

    async fn write_batch(&self, items: Vec<T>) -> Result<()> {
        let encoded = items
            .iter()
            .map(serde_json::to_string)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(dataset_error)?;
        if encoded.is_empty() {
            return Ok(());
        }
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(dataset_error)?;
        for payload in encoded {
            sqlx::query("INSERT INTO spire_dataset_items (dataset, payload) VALUES ($1, $2)")
                .bind(&*self.name)
                .bind(payload)
                .execute(&mut *transaction)
                .await
                .map_err(dataset_error)?;
        }
        transaction.commit().await.map_err(dataset_error)
    }

    async fn read(&self) -> Result<Option<T>> {
        Ok(self.read_batch(1).await?.pop())
    }

    async fn read_batch(&self, max: usize) -> Result<Vec<T>> {
        if max == 0 {
            return Ok(Vec::new());
        }
        let limit = i64::try_from(max).unwrap_or(i64::MAX);
        // Reserve the writer lock before reading, avoiding a deferred
        // transaction's competing read-to-write upgrades across pools.
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(dataset_error)?;
        let rows = sqlx::query(
            "SELECT id, payload FROM spire_dataset_items WHERE dataset = $1 ORDER BY id LIMIT $2",
        )
        .bind(&*self.name)
        .bind(limit)
        .fetch_all(&mut *transaction)
        .await
        .map_err(dataset_error)?;
        let mut items = Vec::with_capacity(rows.len());
        let mut last = None;
        for row in rows {
            let payload: String = row.try_get("payload").map_err(dataset_error)?;
            items.push(serde_json::from_str(&payload).map_err(dataset_error)?);
            last = Some(row.try_get::<i64, _>("id").map_err(dataset_error)?);
        }
        if let Some(last) = last {
            sqlx::query("DELETE FROM spire_dataset_items WHERE dataset = $1 AND id <= $2")
                .bind(&*self.name)
                .bind(last)
                .execute(&mut *transaction)
                .await
                .map_err(dataset_error)?;
        }
        transaction.commit().await.map_err(dataset_error)?;
        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::time::Duration;

    use spire_core::ErrorKind;
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
    use tempfile::TempDir;

    use super::*;
    use crate::sqlite::SqliteStore;

    async fn pool(path: &std::path::Path) -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(path)
                    .create_if_missing(true)
                    .journal_mode(SqliteJournalMode::Wal)
                    .busy_timeout(Duration::from_secs(10)),
            )
            .await
            .unwrap()
    }

    async fn store() -> (TempDir, SqliteStore) {
        let directory = TempDir::new().unwrap();
        let store = SqliteStore::new(pool(&directory.path().join("test.sqlite")).await);
        store.initialize().await.unwrap();
        (directory, store)
    }

    #[tokio::test]
    async fn fifo_batches_and_reopening() {
        let (directory, store) = store().await;
        let data = store.dataset::<String>("titles");
        assert_eq!(data.size_hint(), None);
        assert!(data.is_empty().await.unwrap());
        data.write("a".into()).await.unwrap();
        data.clone()
            .write_batch(vec!["b".into(), "c".into()])
            .await
            .unwrap();
        assert_eq!(data.read_batch(0).await.unwrap(), Vec::<String>::new());
        assert_eq!(data.read().await.unwrap().as_deref(), Some("a"));
        store.pool().close().await;

        let reopened = SqliteStore::new(pool(&directory.path().join("test.sqlite")).await);
        reopened.initialize().await.unwrap();
        let data = reopened.dataset::<String>("titles");
        assert_eq!(data.len().await.unwrap(), 2);
        assert_eq!(data.read_batch(usize::MAX).await.unwrap(), ["b", "c"]);
        assert_eq!(data.read().await.unwrap(), None);
        data.write_batch(Vec::new()).await.unwrap();
        reopened.pool().close().await;
    }

    #[tokio::test]
    async fn dataset_names_are_values_and_do_not_change_schema() {
        let (_directory, store) = store().await;
        let name = "products'); DROP TABLE spire_dataset_items; --";
        store.dataset::<u32>(name).write(1).await.unwrap();
        store.dataset::<u32>("other").write(2).await.unwrap();
        assert_eq!(store.dataset::<u32>(name).read().await.unwrap(), Some(1));
        assert_eq!(store.dataset::<u32>("other").read().await.unwrap(), Some(2));
        store.pool().close().await;
    }

    #[tokio::test]
    async fn serialization_and_sql_failures_leave_whole_write_batch_unwritten() {
        let (_directory, store) = store().await;
        let maps = store.dataset::<HashMap<(u8, u8), u8>>("maps");
        assert!(
            maps.write_batch(vec![HashMap::new(), HashMap::from([((1, 2), 3)])])
                .await
                .is_err()
        );
        assert!(maps.is_empty().await.unwrap());
        sqlx::raw_sql(
            "CREATE TRIGGER reject_item BEFORE INSERT ON spire_dataset_items
            WHEN NEW.payload = '2' BEGIN SELECT RAISE(ABORT, 'rejected'); END;",
        )
        .execute(store.pool())
        .await
        .unwrap();
        let numbers = store.dataset::<u32>("numbers");
        numbers.write(0).await.unwrap();
        assert_eq!(
            numbers.write_batch(vec![1, 2, 3]).await.unwrap_err().kind(),
            ErrorKind::Dataset
        );
        assert_eq!(numbers.read_batch(10).await.unwrap(), [0]);
        store.pool().close().await;
    }

    #[tokio::test]
    async fn decoding_failure_keeps_every_row_until_repaired() {
        let (_directory, store) = store().await;
        let numbers = store.dataset::<u32>("mixed");
        numbers.write(1).await.unwrap();
        store
            .dataset::<String>("mixed")
            .write("bad".into())
            .await
            .unwrap();
        numbers.write(3).await.unwrap();
        assert_eq!(
            numbers.read_batch(10).await.unwrap_err().kind(),
            ErrorKind::Dataset
        );
        assert_eq!(numbers.len().await.unwrap(), 3);
        assert_eq!(numbers.read().await.unwrap(), Some(1));
        assert!(numbers.read().await.is_err());
        assert_eq!(numbers.len().await.unwrap(), 2);
        sqlx::query(
            "UPDATE spire_dataset_items SET payload = '2' WHERE dataset = $1 AND payload = $2",
        )
        .bind("mixed")
        .bind("\"bad\"")
        .execute(store.pool())
        .await
        .unwrap();
        assert_eq!(numbers.read_batch(10).await.unwrap(), [2, 3]);
        store.pool().close().await;
    }

    #[tokio::test]
    async fn failed_delete_rolls_back_read_batch() {
        let (_directory, store) = store().await;
        let numbers = store.dataset::<u32>("numbers");
        numbers.write_batch(vec![1, 2]).await.unwrap();
        sqlx::raw_sql(
            "CREATE TRIGGER reject_delete BEFORE DELETE ON spire_dataset_items
            WHEN OLD.payload = '2' BEGIN SELECT RAISE(ABORT, 'rejected'); END;",
        )
        .execute(store.pool())
        .await
        .unwrap();
        assert!(numbers.read_batch(10).await.is_err());
        assert_eq!(numbers.len().await.unwrap(), 2);
        sqlx::query("DROP TRIGGER reject_delete")
            .execute(store.pool())
            .await
            .unwrap();
        assert_eq!(numbers.read_batch(10).await.unwrap(), [1, 2]);
        store.pool().close().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn independent_pools_preserve_batch_order_and_never_deliver_duplicates() {
        let (directory, first) = store().await;
        let second = SqliteStore::new(pool(&directory.path().join("test.sqlite")).await);
        let mut writers = Vec::new();
        for writer in 0..8_u32 {
            let data = if writer % 2 == 0 { &first } else { &second }.dataset::<u32>("numbers");
            writers.push(tokio::spawn(async move {
                data.write_batch((writer * 20..writer * 20 + 20).collect())
                    .await
                    .unwrap();
            }));
        }
        for writer in writers {
            writer.await.unwrap();
        }
        let snapshot: Vec<(i64, String)> =
            sqlx::query_as("SELECT id, payload FROM spire_dataset_items ORDER BY id")
                .fetch_all(first.pool())
                .await
                .unwrap();
        for batch in snapshot.chunks(20) {
            let values = batch
                .iter()
                .map(|(_, value)| value.parse::<u32>().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(values, (values[0]..values[0] + 20).collect::<Vec<_>>());
        }
        let mut readers = Vec::new();
        for consumer in 0..8 {
            let data = if consumer % 2 == 0 { &first } else { &second }.dataset::<u32>("numbers");
            readers.push(tokio::spawn(async move {
                let mut taken = Vec::new();
                loop {
                    let batch = data.read_batch(7).await.unwrap();
                    if batch.is_empty() {
                        break;
                    }
                    taken.extend(batch);
                }
                taken
            }));
        }
        let mut seen = HashSet::new();
        for reader in readers {
            for value in reader.await.unwrap() {
                assert!(seen.insert(value));
            }
        }
        assert_eq!(seen, (0..160).collect());
        assert!(first.dataset::<u32>("numbers").is_empty().await.unwrap());
        first.pool().close().await;
        second.pool().close().await;
    }

    #[tokio::test]
    async fn missing_schema_and_closed_pool_are_dataset_errors() {
        let directory = TempDir::new().unwrap();
        let store = SqliteStore::new(pool(&directory.path().join("test.sqlite")).await);
        let data = store.dataset::<u32>("numbers");
        assert_eq!(data.write(1).await.unwrap_err().kind(), ErrorKind::Dataset);
        store.initialize().await.unwrap();
        store.pool().close().await;
        assert_eq!(data.read().await.unwrap_err().kind(), ErrorKind::Dataset);
        assert_eq!(data.len().await.unwrap_err().kind(), ErrorKind::Dataset);
    }
    #[tokio::test]
    async fn cancelling_a_partially_executed_batch_rolls_back_and_releases_connection() {
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
        let store = SqliteStore::new(pool.clone());
        store.initialize().await.unwrap();
        let entered = Arc::new(tokio::sync::Notify::new());
        let (release, wait) = std::sync::mpsc::channel();
        {
            let mut connection = pool.acquire().await.unwrap();
            let entered = entered.clone();
            let mut paused = false;
            connection
                .lock_handle()
                .await
                .unwrap()
                .set_update_hook(move |update| {
                    if update.table == "spire_dataset_items" && !paused {
                        paused = true;
                        entered.notify_one();
                        // Pause SQLx's dedicated SQLite worker, not the async runtime.
                        let _ = wait.recv();
                    }
                });
        }
        let data = store.dataset::<u32>("numbers");
        let task = tokio::spawn({
            let data = data.clone();
            async move { data.write_batch(vec![1, 2, 3]).await }
        });
        tokio::time::timeout(Duration::from_secs(5), entered.notified())
            .await
            .unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        release.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            assert!(data.is_empty().await.unwrap());
            data.write(4).await.unwrap();
            assert_eq!(data.read().await.unwrap(), Some(4));
        })
        .await
        .unwrap();
        pool.close().await;
    }
}
