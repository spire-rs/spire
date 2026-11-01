use spire_core::Result;
use sqlx::migrate::{Migration, MigrationType, Migrator};
use sqlx::{SqlSafeStr, SqlitePool};

use crate::error::dataset_error;

fn migrator() -> Migrator {
    let mut migrator = Migrator::with_migrations(vec![Migration::new(
        1,
        "crawl storage".into(),
        MigrationType::Simple,
        include_str!("../../migrations/sqlite/0001_crawl_storage.sql").into_sql_str(),
        false,
    )]);
    // This name is fixed from the first release, not changed on live databases.
    migrator.dangerous_set_table_name("_spire_sqlx_migrations");
    migrator
}

pub(super) async fn run(pool: &SqlitePool) -> Result<()> {
    // Serialize concurrent initializers and include migration history in the
    // same outer transaction. SQLx's per-migration transactions use savepoints.
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(dataset_error)?;
    // Use the existing connection to avoid Acquire's Send inference limitation.
    migrator()
        .run_direct(None, &mut *transaction, false)
        .await
        .map_err(dataset_error)?;
    transaction.commit().await.map_err(dataset_error)
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use tempfile::TempDir;

    use crate::sqlite::SqliteStore;

    async fn store() -> (TempDir, SqliteStore) {
        let directory = TempDir::new().unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(directory.path().join("test.sqlite"))
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        (directory, SqliteStore::new(pool))
    }

    #[tokio::test]
    async fn concurrent_initialization_records_one_version_and_leaves_application_history_alone() {
        let (_directory, store) = store().await;
        sqlx::raw_sql("CREATE TABLE _sqlx_migrations (application TEXT); INSERT INTO _sqlx_migrations VALUES ('retained');")
            .execute(store.pool()).await.unwrap();
        let other = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(store.pool().connect_options().as_ref().clone())
            .await
            .unwrap();
        let second = SqliteStore::new(other.clone());
        let first = tokio::spawn({
            let store = store.clone();
            async move { store.initialize().await }
        });
        second.initialize().await.unwrap();
        first.await.unwrap().unwrap();
        let versions: Vec<i64> = sqlx::query_scalar("SELECT version FROM _spire_sqlx_migrations")
            .fetch_all(store.pool())
            .await
            .unwrap();
        assert_eq!(versions, [1]);
        let app: String = sqlx::query_scalar("SELECT application FROM _sqlx_migrations")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(app, "retained");
        store.pool().close().await;
        other.close().await;
    }

    #[tokio::test]
    async fn failure_halfway_through_migration_rolls_back_schema_and_history() {
        let (_directory, store) = store().await;
        // This collision occurs after the dataset table and index are created.
        sqlx::query("CREATE TABLE spire_seen (application TEXT)")
            .execute(store.pool())
            .await
            .unwrap();
        assert!(store.initialize().await.is_err());
        let created: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_schema WHERE name IN ('spire_dataset_items', 'spire_dataset_order', '_spire_sqlx_migrations')")
            .fetch_one(store.pool()).await.unwrap();
        assert_eq!(created, 0);
        sqlx::query("INSERT INTO spire_seen (application) VALUES ('untouched')")
            .execute(store.pool())
            .await
            .unwrap();
        store.pool().close().await;
    }

    #[tokio::test]
    async fn changed_checksums_and_unknown_versions_are_rejected_without_losing_data() {
        use spire_core::dataset::Dataset;

        let (_directory, store) = store().await;
        store.initialize().await.unwrap();
        let data = store.dataset::<String>("records");
        data.write("retained".into()).await.unwrap();
        let checksum: Vec<u8> =
            sqlx::query_scalar("SELECT checksum FROM _spire_sqlx_migrations WHERE version = 1")
                .fetch_one(store.pool())
                .await
                .unwrap();
        sqlx::query("UPDATE _spire_sqlx_migrations SET checksum = X'00' WHERE version = 1")
            .execute(store.pool())
            .await
            .unwrap();
        assert!(store.initialize().await.is_err());
        sqlx::query("UPDATE _spire_sqlx_migrations SET checksum = $1 WHERE version = 1")
            .bind(checksum)
            .execute(store.pool())
            .await
            .unwrap();
        sqlx::query("UPDATE _spire_sqlx_migrations SET version = 999 WHERE version = 1")
            .execute(store.pool())
            .await
            .unwrap();
        assert!(store.initialize().await.is_err());
        assert_eq!(data.read().await.unwrap().as_deref(), Some("retained"));
        store.pool().close().await;
    }
}
