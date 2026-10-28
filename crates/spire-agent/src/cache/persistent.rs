use futures::future::BoxFuture;
use spire_redb::RedbStore;
use spire_redb::redb::{TableDefinition, TableError};

use super::{AgentStoreExt, Cache};
use crate::{Error, Result};

const ANSWERS: TableDefinition<&str, &str> = TableDefinition::new("spire-agent/answers/v1");

/// An answer cache sharing a crawl's [`RedbStore`], including its durability
/// policy. Blocking reads use Tokio's blocking pool; writes use the store's
/// own transactional write path.
#[derive(Debug, Clone)]
pub struct RedbCache {
    store: RedbStore,
}

impl AgentStoreExt for RedbStore {
    type Cache = RedbCache;

    /// Returns a cache whose table is created on the first answer write.
    fn agent_cache(&self) -> Self::Cache {
        RedbCache {
            store: self.clone(),
        }
    }
}

fn failed(error: impl std::fmt::Display) -> Error {
    Error::Cache(error.to_string())
}

impl Cache for RedbCache {
    fn get<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Result<Option<String>>> {
        Box::pin(async move {
            let store = self.store.clone();
            let key = key.to_owned();
            tokio::task::spawn_blocking(move || {
                store.read(|transaction| {
                    let table = match transaction.open_table(ANSWERS) {
                        Ok(table) => table,
                        Err(TableError::TableDoesNotExist(_)) => return Ok(None),
                        Err(error) => return Err(error.into()),
                    };
                    Ok(table
                        .get(key.as_str())?
                        .map(|value| value.value().to_owned()))
                })
            })
            .await
            .map_err(failed)?
            .map_err(failed)
        })
    }

    fn put<'a>(&'a self, key: &'a str, answer: &'a str) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            let key = key.to_owned();
            let answer = answer.to_owned();
            self.store
                .write(move |transaction| {
                    transaction
                        .open_table(ANSWERS)?
                        .insert(key.as_str(), answer.as_str())?;
                    Ok(())
                })
                .await
                .map_err(failed)
        })
    }
}
