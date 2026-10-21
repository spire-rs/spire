use std::collections::HashSet;
use std::future::{Future, ready};
use std::sync::Mutex;

use crate::Result;
use crate::frontier::seed::Fingerprint;
use crate::frontier::seen::SeenSet;
use crate::sync::lock;

/// A [`SeenSet`] kept in memory for the lifetime of the crawl.
///
/// It holds every fingerprint pushed, about 16 bytes each: a million URLs
/// take roughly 16 MB. Capping it would let duplicates through; for larger
/// crawls use [`BloomSeen`](crate::frontier::seen::BloomSeen), which takes fixed
/// memory at the cost of rarely skipping a new URL, or a persistent
/// [`SeenSet`].
#[derive(Debug, Default)]
pub struct InMemSeen {
    seen: Mutex<HashSet<Fingerprint>>,
}

impl InMemSeen {
    /// Returns every fingerprint seen, e.g. to save the crawl.
    #[must_use]
    pub fn snapshot(&self) -> Vec<Fingerprint> {
        lock(&self.seen).iter().copied().collect()
    }
}

impl FromIterator<Fingerprint> for InMemSeen {
    /// Collects fingerprints, e.g. to restore a snapshot.
    fn from_iter<I: IntoIterator<Item = Fingerprint>>(fingerprints: I) -> Self {
        Self {
            seen: Mutex::new(fingerprints.into_iter().collect()),
        }
    }
}

impl SeenSet for InMemSeen {
    fn insert(&self, fingerprint: Fingerprint) -> impl Future<Output = Result<bool>> + Send {
        ready(Ok(lock(&self.seen).insert(fingerprint)))
    }
}
