use std::fmt;

use spire_core::Result;
use spire_core::frontier::seed::Fingerprint;
use spire_core::frontier::seen::SeenSet;

use crate::store::{RedbStore, SEEN};

/// A [`SeenSet`] stored in a [`RedbStore`], exact and on disk: memory stays
/// flat however many URLs a crawl sees.
pub struct RedbSeen {
    store: RedbStore,
}

impl RedbSeen {
    pub(crate) const fn new(store: RedbStore) -> Self {
        Self { store }
    }

    /// Returns the number of fingerprints seen.
    ///
    /// # Errors
    ///
    /// Returns a [`Frontier`](spire_core::ErrorKind::Frontier) error if the
    /// store cannot be read.
    pub async fn len(&self) -> Result<usize> {
        self.store.blocking(|store| store.count(SEEN)).await
    }

    /// Whether nothing was seen yet.
    ///
    /// # Errors
    ///
    /// Returns a [`Frontier`](spire_core::ErrorKind::Frontier) error if the
    /// store cannot be read.
    pub async fn is_empty(&self) -> Result<bool> {
        Ok(self.len().await? == 0)
    }
}

impl fmt::Debug for RedbSeen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedbSeen")
            .field("store", &self.store)
            .finish()
    }
}

impl SeenSet for RedbSeen {
    async fn contains(&self, fingerprint: Fingerprint) -> Result<bool> {
        let key = fingerprint.get();
        self.store
            .blocking(move |store| {
                store.read(|txn| match txn.open_table(SEEN) {
                    Ok(seen) => Ok(seen.get(key)?.is_some()),
                    // Tables are created by their first write.
                    Err(redb::TableError::TableDoesNotExist(_)) => Ok(false),
                    Err(error) => Err(error.into()),
                })
            })
            .await
    }

    async fn insert(&self, fingerprint: Fingerprint) -> Result<bool> {
        let key = fingerprint.get();
        self.store
            .write(move |txn| {
                let mut seen = txn.open_table(SEEN)?;
                Ok(seen.insert(key, ())?.is_none())
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontier::tests::{TempPath, seed};

    #[tokio::test]
    async fn remembers_across_reopening() {
        let path = TempPath::new();
        {
            let seen = RedbStore::open(&path.0).await.unwrap().seen();
            assert!(!seen.contains(seed("a").fingerprint()).await.unwrap());
            assert!(seen.insert(seed("a").fingerprint()).await.unwrap());
            assert!(seen.contains(seed("a").fingerprint()).await.unwrap());
            assert!(!seen.insert(seed("a").fingerprint()).await.unwrap());
        }

        let seen = RedbStore::open(&path.0).await.unwrap().seen();
        assert_eq!(seen.len().await.unwrap(), 1);
        assert!(!seen.insert(seed("a").fingerprint()).await.unwrap());
        assert!(seen.insert(seed("b").fingerprint()).await.unwrap());
    }
}
