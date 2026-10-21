//! Seen sets: what a [`Dedup`](crate::frontier::Dedup) frontier remembers.

mod bloom;
mod memory;

use std::future::Future;

pub use bloom::{BloomSeen, BloomSnapshot};
pub use memory::InMemSeen;

use crate::Result;
use crate::frontier::Fingerprint;

/// Remembers which [`Fingerprint`]s a [`Dedup`](crate::frontier::Dedup) frontier has seen.
pub trait SeenSet: Send + Sync + 'static {
    /// Records `fingerprint`, returning `true` if it was not seen before.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying storage fails.
    fn insert(&self, fingerprint: Fingerprint) -> impl Future<Output = Result<bool>> + Send;
}

impl<S: SeenSet> SeenSet for std::sync::Arc<S> {
    fn insert(&self, fingerprint: Fingerprint) -> impl Future<Output = Result<bool>> + Send {
        S::insert(self, fingerprint)
    }
}
