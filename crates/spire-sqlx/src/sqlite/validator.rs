use spire_core::Result;
use spire_core::frontier::seed::Fingerprint;
use spire_core::http::http::HeaderValue;
use spire_core::worker::middleware::{PageVersion, Validator};
use sqlx::{Row, SqlitePool};

use crate::error::frontier_error;

/// Page versions persisted as raw header bytes in SQLite.
///
/// Handles share records across connections and restarts. Initialize the store
/// before using them. Non-UTF-8 header values are preserved without conversion.
#[derive(Debug, Clone)]
pub struct SqliteValidator {
    pool: SqlitePool,
}

impl SqliteValidator {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Counts the stored page versions.
    ///
    /// # Errors
    /// Returns a frontier error if the query or count conversion fails.
    pub async fn len(&self) -> Result<usize> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM spire_page_versions")
            .fetch_one(&self.pool)
            .await
            .map_err(frontier_error)?;
        usize::try_from(count).map_err(frontier_error)
    }

    /// Whether no versions are stored.
    ///
    /// # Errors
    /// Returns a frontier error if the query fails.
    pub async fn is_empty(&self) -> Result<bool> {
        let present: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM spire_page_versions)")
            .fetch_one(&self.pool)
            .await
            .map_err(frontier_error)?;
        Ok(!present)
    }
}

impl Validator for SqliteValidator {
    async fn get(&self, request: Fingerprint) -> Result<Option<PageVersion>> {
        let row = sqlx::query(
            "SELECT etag, last_modified FROM spire_page_versions WHERE fingerprint = $1",
        )
        .bind(request.get().to_be_bytes().to_vec())
        .fetch_optional(&self.pool)
        .await
        .map_err(frontier_error)?;

        row.map(|row| {
            let decode = |name| -> Result<Option<HeaderValue>> {
                row.try_get::<Option<Vec<u8>>, _>(name)
                    .map_err(frontier_error)?
                    .map(|bytes| HeaderValue::from_bytes(&bytes).map_err(frontier_error))
                    .transpose()
            };
            Ok(PageVersion {
                etag: decode("etag")?,
                last_modified: decode("last_modified")?,
            })
        })
        .transpose()
    }

    async fn set(&self, request: Fingerprint, version: PageVersion) -> Result<()> {
        sqlx::query("INSERT INTO spire_page_versions (fingerprint, etag, last_modified) VALUES ($1, $2, $3)
            ON CONFLICT(fingerprint) DO UPDATE SET etag = excluded.etag, last_modified = excluded.last_modified")
            .bind(request.get().to_be_bytes().to_vec())
            .bind(version.etag.map(|value| value.as_bytes().to_vec()))
            .bind(version.last_modified.map(|value| value.as_bytes().to_vec()))
            .execute(&self.pool).await.map_err(frontier_error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use spire_core::frontier::seed::Seed;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use tempfile::TempDir;

    use super::*;
    use crate::sqlite::SqliteStore;

    #[tokio::test]
    async fn raw_headers_survive_reopening_and_replacement_clears_old_values() {
        let directory = TempDir::new().unwrap();
        let options = SqliteConnectOptions::new()
            .filename(directory.path().join("test.sqlite"))
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone())
            .await
            .unwrap();
        let store = SqliteStore::new(pool.clone());
        store.initialize().await.unwrap();
        let key = Seed::get("https://example.com/").unwrap().fingerprint();
        let value = PageVersion {
            etag: Some(HeaderValue::from_bytes(b"\"v\xff1\"").unwrap()),
            last_modified: Some(HeaderValue::from_static("Wed, 21 Oct 2026 07:28:00 GMT")),
        };
        assert!(store.validator().is_empty().await.unwrap());
        assert_eq!(store.validator().get(key).await.unwrap(), None);
        store.validator().set(key, value.clone()).await.unwrap();
        pool.close().await;
        let reopened = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let validator = SqliteStore::new(reopened.clone()).validator();
        assert_eq!(validator.get(key).await.unwrap(), Some(value));
        validator.set(key, PageVersion::default()).await.unwrap();
        assert_eq!(validator.len().await.unwrap(), 1);
        assert_eq!(
            validator.get(key).await.unwrap(),
            Some(PageVersion::default())
        );
        reopened.close().await;
    }
}
