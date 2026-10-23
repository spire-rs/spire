use std::fmt;
use std::future::{Future, ready};

use spire_core::Result;
use spire_core::frontier::seed::Fingerprint;
use spire_core::http::HeaderValue;
use spire_core::worker::middleware::{Validator, Validators};

use crate::store::{RedbStore, VALIDATORS};

/// [`Validators`] stored in a [`RedbStore`], so later runs revalidate the
/// pages earlier ones crawled.
pub struct RedbValidators {
    store: RedbStore,
}

impl RedbValidators {
    pub(crate) const fn new(store: RedbStore) -> Self {
        Self { store }
    }

    /// Returns the number of pages with a stored validator.
    ///
    /// # Errors
    ///
    /// Returns a [`Frontier`](spire_core::ErrorKind::Frontier) error if the
    /// store cannot be read.
    pub fn len(&self) -> Result<usize> {
        self.store.count(VALIDATORS)
    }

    /// Whether no validator is stored.
    ///
    /// # Errors
    ///
    /// Returns a [`Frontier`](spire_core::ErrorKind::Frontier) error if the
    /// store cannot be read.
    pub fn is_empty(&self) -> Result<bool> {
        self.len().map(|len| len == 0)
    }
}

impl Clone for RedbValidators {
    fn clone(&self) -> Self {
        Self::new(self.store.clone())
    }
}

impl fmt::Debug for RedbValidators {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedbValidators")
            .field("store", &self.store)
            .finish()
    }
}

/// Encodes a validator as its two header values, each a presence byte, a
/// length and the raw bytes, so values that are not text survive.
fn encode(validator: &Validator) -> Vec<u8> {
    let mut bytes = Vec::new();
    for value in [&validator.etag, &validator.last_modified] {
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

/// Decodes a validator, or `None` if the bytes are not one.
fn decode(mut bytes: &[u8]) -> Option<Validator> {
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
    Some(Validator {
        etag,
        last_modified,
    })
}

impl Validators for RedbValidators {
    fn get(&self, request: Fingerprint) -> impl Future<Output = Result<Option<Validator>>> + Send {
        let key = request.get();
        let bytes = self.store.read(|txn| {
            let table = match txn.open_table(VALIDATORS) {
                Ok(table) => table,
                Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
                Err(error) => return Err(error.into()),
            };
            Ok(table.get(key)?.map(|bytes| bytes.value().to_vec()))
        });
        ready(bytes.map(|bytes| bytes.as_deref().and_then(decode)))
    }

    async fn set(&self, request: Fingerprint, validator: Validator) -> Result<()> {
        let key = request.get();
        let bytes = encode(&validator);
        self.store
            .write(move |txn| {
                txn.open_table(VALIDATORS)?.insert(key, bytes.as_slice())?;
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
    fn validators_round_trip_including_bytes_that_are_not_text() {
        let validator = Validator {
            etag: Some(HeaderValue::from_bytes(b"\"v\xff1\"").unwrap()),
            last_modified: None,
        };
        assert_eq!(decode(&encode(&validator)), Some(validator));
        assert_eq!(
            decode(&encode(&Validator::default())),
            Some(Validator::default())
        );
        assert_eq!(decode(&[1, 9]), None, "truncated");
    }

    #[tokio::test]
    async fn validators_survive_reopening() {
        let path = TempPath::new();
        let key = seed("a").fingerprint();
        let validator = Validator {
            etag: Some(HeaderValue::from_static("\"v1\"")),
            last_modified: Some(HeaderValue::from_static("Wed, 21 Oct 2026 07:28:00 GMT")),
        };
        {
            let validators = RedbStore::open(&path.0).await.unwrap().validators();
            assert_eq!(validators.get(key).await.unwrap(), None);
            validators.set(key, validator.clone()).await.unwrap();
        }

        let validators = RedbStore::open(&path.0).await.unwrap().validators();
        assert_eq!(validators.get(key).await.unwrap(), Some(validator));
        assert_eq!(validators.len().unwrap(), 1);
    }
}
