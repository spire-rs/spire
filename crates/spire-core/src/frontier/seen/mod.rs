//! Seen sets: what a [`Dedup`](crate::frontier::facade::Dedup) frontier
//! remembers.

mod bloom;
mod memory;

use std::future::Future;

pub use bloom::{BloomSeen, BloomSnapshot};
pub use memory::InMemSeen;

use crate::Result;
use crate::frontier::seed::Fingerprint;

/// Remembers which [`Fingerprint`]s a [`Dedup`](crate::frontier::facade::Dedup)
/// frontier has seen.
///
/// `Dedup` checks [`contains`](Self::contains) before queueing a seed and
/// records it with [`insert`](Self::insert) only once it is queued, so a crash
/// in between can queue it again, but never loses it. A seen set should last
/// no longer than its frontier's queue: one kept across a restart whose queue
/// was not loses the seeds that were queued, as they stay recorded as seen.
pub trait SeenSet: Send + Sync + 'static {
    /// Whether `fingerprint` was recorded, without recording it.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying storage fails.
    fn contains(&self, fingerprint: Fingerprint) -> impl Future<Output = Result<bool>> + Send;

    /// Records `fingerprint`, returning `true` if it was not seen before.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying storage fails.
    fn insert(&self, fingerprint: Fingerprint) -> impl Future<Output = Result<bool>> + Send;
}

impl<S: SeenSet> SeenSet for std::sync::Arc<S> {
    fn contains(&self, fingerprint: Fingerprint) -> impl Future<Output = Result<bool>> + Send {
        S::contains(self, fingerprint)
    }

    fn insert(&self, fingerprint: Fingerprint) -> impl Future<Output = Result<bool>> + Send {
        S::insert(self, fingerprint)
    }
}
