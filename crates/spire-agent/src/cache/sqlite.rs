use std::sync::Arc;

use futures::future::BoxFuture;
use spire_sqlx::sqlite::SqliteStore;
use spire_sqlx::sqlx;
use spire_sqlx::sqlx::SqlSafeStr;
use spire_sqlx::sqlx::migrate::{Migration, MigrationType, Migrator};
use tokio::sync::OnceCell;

use super::{AgentStoreExt, Cache};
use crate::{Error, Result};

/// Validated model answers sharing a SQLite store's connection pool.
///
/// The first operation migrates the reserved `spire_agent_answers` table,
/// tracking versions in `_spire_agent_migrations`. Clones share initialization.
/// Pool durability settings are unchanged.
/// Attach this cache explicitly with [`crate::Agent::with_cache`].
#[derive(Debug, Clone)]
pub struct SqliteCache {
    store: SqliteStore,
    initialized: Arc<OnceCell<()>>,
}

impl AgentStoreExt for SqliteStore {
    type Cache = SqliteCache;

    fn agent_cache(&self) -> Self::Cache {
        SqliteCache {
            store: self.clone(),
            initialized: Arc::new(OnceCell::new()),
        }
    }
}

fn failed(error: impl std::fmt::Display) -> Error {
    Error::Cache(error.to_string())
}

impl SqliteCache {
    async fn initialize(&self) -> Result<()> {
        self.initialized
            .get_or_try_init(|| async {
                let mut migrator = Migrator::with_migrations(vec![Migration::new(
                    1,
                    "agent answers".into(),
                    MigrationType::Simple,
                    include_str!("../../migrations/sqlite/0001_agent_answers.sql").into_sql_str(),
                    false,
                )]);
                migrator.dangerous_set_table_name("_spire_agent_migrations");
                let mut transaction = self
                    .store
                    .pool()
                    .begin_with("BEGIN IMMEDIATE")
                    .await
                    .map_err(failed)?;
                // Run on the existing connection. The generic Acquire entry point
                // cannot produce a Send future here on our supported Rust version.
                migrator
                    .run_direct(None, &mut *transaction, false)
                    .await
                    .map_err(failed)?;
                transaction.commit().await.map_err(failed)
            })
            .await
            .copied()
    }
}

impl Cache for SqliteCache {
    fn get<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Result<Option<String>>> {
        Box::pin(async move {
            self.initialize().await?;
            sqlx::query_scalar("SELECT answer FROM spire_agent_answers WHERE key = $1")
                .bind(key)
                .fetch_optional(self.store.pool())
                .await
                .map_err(failed)
        })
    }

    fn put<'a>(&'a self, key: &'a str, answer: &'a str) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            self.initialize().await?;
            sqlx::query(
                "INSERT INTO spire_agent_answers (key, answer) VALUES ($1, $2)
                ON CONFLICT(key) DO UPDATE SET answer = excluded.answer",
            )
            .bind(key)
            .bind(answer)
            .execute(self.store.pool())
            .await
            .map_err(failed)?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use spire_sqlx::sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    use super::*;
    use crate::testing::Script;
    use crate::{Agent, ProviderResponse, Usage};

    #[tokio::test]
    async fn persistent_answers_are_used_after_reopening_without_provider_calls() {
        let directory = tempfile::TempDir::new().unwrap();
        let options = SqliteConnectOptions::new()
            .filename(directory.path().join("cache.sqlite"))
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(options.clone())
            .await
            .unwrap();
        let store = SqliteStore::new(pool.clone());
        let cache = store.agent_cache();
        assert_eq!(cache.get("missing").await.unwrap(), None);
        let a = store.agent_cache();
        let b = store.agent_cache();
        let (one, two) = tokio::join!(a.put("one", "first"), b.put("two", "second"));
        one.unwrap();
        two.unwrap();
        assert_eq!(cache.get("one").await.unwrap().as_deref(), Some("first"));
        cache.put("one", "replaced").await.unwrap();
        assert_eq!(cache.get("one").await.unwrap().as_deref(), Some("replaced"));
        let provider = Script::new([ProviderResponse {
            text: r#"{"result":"Basic"}"#.into(),
            usage: Usage::tokens(2, 1),
        }]);
        let agent = Agent::new(provider.clone()).with_cache(cache.clone(), "test/model");
        assert_eq!(agent.extract::<String>("plans").await.unwrap(), "Basic");
        assert_eq!(provider.calls(), 1);
        pool.close().await;
        assert!(matches!(cache.get("one").await, Err(Error::Cache(_))));
        let reopened = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let provider = Script::default();
        let agent = Agent::new(provider.clone()).with_cache(
            SqliteStore::new(reopened.clone()).agent_cache(),
            "test/model",
        );
        assert_eq!(agent.extract::<String>("plans").await.unwrap(), "Basic");
        assert_eq!(provider.calls(), 0);
        reopened.close().await;
    }
    #[tokio::test]
    async fn cache_and_store_migration_histories_are_independent_and_checksums_are_validated() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let store = SqliteStore::new(pool.clone());
        let cache = store.agent_cache();
        cache.put("key", "retained").await.unwrap();
        store.initialize().await.unwrap();
        for query in [
            "SELECT COUNT(*) FROM _spire_agent_migrations",
            "SELECT COUNT(*) FROM _spire_sqlx_migrations",
        ] {
            let count: i64 = sqlx::query_scalar(query).fetch_one(&pool).await.unwrap();
            assert_eq!(count, 1);
        }
        sqlx::query("UPDATE _spire_agent_migrations SET checksum = X'00' WHERE version = 1")
            .execute(&pool)
            .await
            .unwrap();
        assert!(matches!(
            store.agent_cache().get("key").await,
            Err(Error::Cache(_))
        ));
        assert_eq!(cache.get("key").await.unwrap().as_deref(), Some("retained"));
        pool.close().await;
    }
}
