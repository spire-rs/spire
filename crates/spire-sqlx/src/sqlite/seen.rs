use spire_core::Result;
use spire_core::frontier::seed::Fingerprint;
use spire_core::frontier::seen::SeenSet;
use sqlx::SqlitePool;

use crate::error::frontier_error;

/// A persistent exact seen set shared across SQLite connections.
///
/// Fingerprints are stored as eight-byte blobs, preserving the entire unsigned
/// 64-bit range. Atomic inserts ensure only one concurrent caller reports a
/// fingerprint as new. Initialize the store before using this handle.
#[derive(Debug, Clone)]
pub struct SqliteSeen {
    pool: SqlitePool,
}

impl SqliteSeen {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Counts the fingerprints currently stored.
    ///
    /// # Errors
    /// Returns a frontier error if the query or count conversion fails.
    pub async fn len(&self) -> Result<usize> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM spire_seen")
            .fetch_one(&self.pool)
            .await
            .map_err(frontier_error)?;
        usize::try_from(count).map_err(frontier_error)
    }

    /// Whether no fingerprints are stored.
    ///
    /// # Errors
    /// Returns a frontier error if the query fails.
    pub async fn is_empty(&self) -> Result<bool> {
        let present: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM spire_seen)")
            .fetch_one(&self.pool)
            .await
            .map_err(frontier_error)?;
        Ok(!present)
    }
}

impl SeenSet for SqliteSeen {
    async fn insert(&self, fingerprint: Fingerprint) -> Result<bool> {
        let result = sqlx::query(
            "INSERT INTO spire_seen (fingerprint) VALUES ($1) ON CONFLICT(fingerprint) DO NOTHING",
        )
        .bind(fingerprint.get().to_be_bytes().to_vec())
        .execute(&self.pool)
        .await
        .map_err(frontier_error)?;
        Ok(result.rows_affected() == 1)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use spire_core::frontier::seed::Seed;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use tempfile::TempDir;

    use super::*;
    use crate::sqlite::SqliteStore;

    #[tokio::test]
    async fn independent_handles_insert_once_and_persist_unsigned_fingerprints() {
        let directory = TempDir::new().unwrap();
        let options = SqliteConnectOptions::new()
            .filename(directory.path().join("test.sqlite"))
            .create_if_missing(true)
            .busy_timeout(Duration::from_secs(5));
        let first = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone())
            .await
            .unwrap();
        let second = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone())
            .await
            .unwrap();
        let store = SqliteStore::new(first.clone());
        store.initialize().await.unwrap();
        let key = (0..100)
            .map(|n| {
                Seed::get(format!("https://example.com/{n}"))
                    .unwrap()
                    .fingerprint()
            })
            .find(|key| key.get() > u64::try_from(i64::MAX).unwrap())
            .unwrap();
        let a = store.seen();
        let b = SqliteStore::new(second.clone()).seen();
        assert!(a.is_empty().await.unwrap());
        let (a_new, b_new) = tokio::join!(a.insert(key), b.insert(key));
        assert_ne!(a_new.unwrap(), b_new.unwrap());
        assert_eq!(a.len().await.unwrap(), 1);
        first.close().await;
        second.close().await;
        let reopened = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let seen = SqliteStore::new(reopened.clone()).seen();
        assert!(!seen.insert(key).await.unwrap());
        reopened.close().await;
    }
}
