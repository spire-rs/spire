use std::fmt;

use spire_core::Result;
use spire_core::frontier::seed::Fingerprint;
use spire_core::http::http::HeaderValue;
use spire_core::worker::middleware::{PageVersion, Validator};

use crate::store::{PAGE_VERSIONS, RedbStore};

/// [`Validator`] stored in a [`RedbStore`], so later runs revalidate the
/// pages earlier ones crawled.
pub struct RedbValidator {
    store: RedbStore,
}

impl RedbValidator {
    pub(crate) const fn new(store: RedbStore) -> Self {
        Self { store }
    }

    /// Returns the number of pages with a stored version.
    ///
    /// # Errors
    ///
    /// Returns a [`Frontier`](spire_core::ErrorKind::Frontier) error if the
    /// store cannot be read.
    pub async fn len(&self) -> Result<usize> {
        self.store
            .blocking(|store| store.count(PAGE_VERSIONS))
            .await
    }

    /// Whether no version is stored.
    ///
    /// # Errors
    ///
    /// Returns a [`Frontier`](spire_core::ErrorKind::Frontier) error if the
    /// store cannot be read.
    pub async fn is_empty(&self) -> Result<bool> {
        Ok(self.len().await? == 0)
    }
}

impl Clone for RedbValidator {
    fn clone(&self) -> Self {
        Self::new(self.store.clone())
    }
}

impl fmt::Debug for RedbValidator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedbValidator")
            .field("store", &self.store)
            .finish()
    }
}

/// Encodes a version as its two header values, each a presence byte, a
/// length and the raw bytes, so values that are not text survive.
fn encode(version: &PageVersion) -> Vec<u8> {
    let mut bytes = Vec::new();
    for value in [&version.etag, &version.last_modified] {
        match value {
            Some(value) => {
                let value = value.as_bytes();
                bytes.push(1);
                bytes.extend_from_slice(
                    &u32::try_from(value.len()).unwrap_or(u32::MAX).to_le_bytes(),
                );
                bytes.extend_from_slice(value);
            }
            None => bytes.push(0),
        }
    }
    bytes
}

/// Decodes a version, or `None` if the bytes are not one.
fn decode(mut bytes: &[u8]) -> Option<PageVersion> {
    let mut next = || -> Option<Option<HeaderValue>> {
        let (&present, rest) = bytes.split_first()?;
        bytes = rest;
        if present == 0 {
            return Some(None);
        }
        let (len, rest) = bytes.split_first_chunk::<4>()?;
        let len = usize::try_from(u32::from_le_bytes(*len)).ok()?;
        let (value, rest) = rest.split_at_checked(len)?;
        bytes = rest;
        HeaderValue::from_bytes(value).ok().map(Some)
    };
    let etag = next()?;
    let last_modified = next()?;
    Some(PageVersion {
        etag,
        last_modified,
    })
}

impl Validator for RedbValidator {
    async fn get(&self, request: Fingerprint) -> Result<Option<PageVersion>> {
        let key = request.get();
        let bytes = self
            .store
            .blocking(move |store| {
                store.read(|txn| {
                    let table = match txn.open_table(PAGE_VERSIONS) {
                        Ok(table) => table,
                        Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
                        Err(error) => return Err(error.into()),
                    };
                    Ok(table.get(key)?.map(|bytes| bytes.value().to_vec()))
                })
            })
            .await?;
        Ok(bytes.as_deref().and_then(decode))
    }

    async fn set(&self, request: Fingerprint, version: PageVersion) -> Result<()> {
        let key = request.get();
        let bytes = encode(&version);
        self.store
            .write(move |txn| {
                txn.open_table(PAGE_VERSIONS)?
                    .insert(key, bytes.as_slice())?;
                Ok(())
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontier::tests::{TempPath, seed};

    #[test]
    fn versions_round_trip_including_bytes_that_are_not_text() {
        let version = PageVersion {
            etag: Some(HeaderValue::from_bytes(b"\"v\xff1\"").unwrap()),
            last_modified: None,
        };
        assert_eq!(decode(&encode(&version)), Some(version));
        assert_eq!(
            decode(&encode(&PageVersion::default())),
            Some(PageVersion::default())
        );
        assert_eq!(decode(&[1, 9]), None, "truncated");
    }

    #[tokio::test]
    async fn versions_survive_reopening() {
        let path = TempPath::new();
        let key = seed("a").fingerprint();
        let version = PageVersion {
            etag: Some(HeaderValue::from_static("\"v1\"")),
            last_modified: Some(HeaderValue::from_static("Wed, 21 Oct 2026 07:28:00 GMT")),
        };
        {
            let validator = RedbStore::open(&path.0).await.unwrap().validator();
            assert_eq!(validator.get(key).await.unwrap(), None);
            validator.set(key, version.clone()).await.unwrap();
        }

        let validator = RedbStore::open(&path.0).await.unwrap().validator();
        assert_eq!(validator.get(key).await.unwrap(), Some(version));
        assert_eq!(validator.len().await.unwrap(), 1);
    }
}
