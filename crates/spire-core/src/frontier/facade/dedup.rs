use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::sync::{Arc, Mutex};

use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

use crate::Result;
use crate::frontier::seed::{Fingerprint, Seed};
use crate::frontier::seen::{InMemSeen, SeenSet};
use crate::frontier::{Ack, Admission, Frontier, NextSeed, PopMode, Report};
use crate::sync::lock;

/// A facade that refuses seeds seen before, see [`Seed::fingerprint`].
///
/// Retried seeds are not pushed again, so they are never refused.
///
/// A seed is checked with [`SeenSet::contains`], queued, and only then
/// recorded with [`SeenSet::insert`], so a crash in between queues it again
/// when it is found again, rather than refusing it as seen and losing it. A
/// seed the facades it wraps refuse, e.g. as too deep, is not recorded, so
/// found again where they admit it, it is queued. If
/// recording fails after the seed is queued, the push returns the error, and
/// pushing the seed again queues it a second time.
///
/// Pushes of the same seed at once wait for each other, so it is queued once.
/// They are tracked by this facade, so two facades sharing one seen set can
/// each queue it once.
pub struct Dedup<F, S = InMemSeen> {
    inner: F,
    seen: S,
    pushes: Pushes,
}

impl<F, S> Dedup<F, S> {
    /// Wraps `inner`, remembering seeds in `seen`.
    pub fn new(inner: F, seen: S) -> Self {
        Self {
            inner,
            seen,
            pushes: Pushes::default(),
        }
    }
}

/// The fingerprints being pushed, so pushes of the same one take turns.
///
/// Each has a lock, held by its push until it ends and shared with the pushes
/// waiting for it, which release the map before they wait.
#[derive(Default)]
struct Pushes {
    pushing: Mutex<HashMap<Fingerprint, Arc<AsyncMutex<()>>>>,
}

impl Pushes {
    /// Marks `fingerprint` as being pushed, once no other push of it is under
    /// way, until the returned guard is dropped.
    async fn start(&self, fingerprint: Fingerprint) -> Pushing<'_> {
        loop {
            let other = {
                let mut pushing = lock(&self.pushing);
                if let Some(other) = pushing.get(&fingerprint) {
                    other.clone()
                } else {
                    let held = Arc::new(AsyncMutex::new(()));
                    let guard = held
                        .clone()
                        .try_lock_owned()
                        .unwrap_or_else(|_| unreachable!("the lock was just made"));
                    pushing.insert(fingerprint, held.clone());
                    return Pushing {
                        pushes: self,
                        fingerprint,
                        held,
                        _guard: guard,
                    };
                }
            };
            // Released once the other push ends, however it ends; the lock
            // stays released, so this cannot miss it.
            drop(other.lock().await);
        }
    }

    /// Forgets `fingerprint`, unless a later push of it took its place.
    fn finish(&self, fingerprint: Fingerprint, held: &Arc<AsyncMutex<()>>) {
        let mut pushing = lock(&self.pushing);
        if pushing
            .get(&fingerprint)
            .is_some_and(|current| Arc::ptr_eq(current, held))
        {
            pushing.remove(&fingerprint);
        }
    }
}

/// Marks a fingerprint as being pushed until dropped, even if the push is
/// cancelled, and then lets the pushes waiting on it go on.
struct Pushing<'a> {
    pushes: &'a Pushes,
    fingerprint: Fingerprint,
    held: Arc<AsyncMutex<()>>,
    _guard: OwnedMutexGuard<()>,
}

impl Drop for Pushing<'_> {
    fn drop(&mut self) {
        self.pushes.finish(self.fingerprint, &self.held);
        // The guard is released after this, waking the pushes waiting.
    }
}

impl<F: fmt::Debug, S> fmt::Debug for Dedup<F, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Dedup")
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

impl<F: Frontier, S: SeenSet> Frontier for Dedup<F, S> {
    async fn push(&self, seed: Seed) -> Result<Admission> {
        let fingerprint = seed.fingerprint();
        let _pushing = self.pushes.start(fingerprint).await;
        if self.seen.contains(fingerprint).await? {
            return Ok(Admission::Duplicate);
        }
        // Queued before it is recorded: a crash in between queues it again
        // when the page that found it is crawled again, rather than refusing
        // it as seen and losing it.
        let admission = self.inner.push(seed).await?;
        // Refused further in, e.g. too deep, it may be admitted when found
        // again, so it is not seen.
        if admission.is_queued() {
            self.seen.insert(fingerprint).await?;
        }
        Ok(admission)
    }

    fn permits(&self, seed: &Seed) -> bool {
        self.inner.permits(seed)
    }

    fn pop_with(&self, mode: PopMode) -> impl Future<Output = Result<NextSeed>> + Send {
        self.inner.pop_with(mode)
    }

    fn report(&self, report: Report) -> impl Future<Output = Result<Ack>> + Send {
        self.inner.report(report)
    }

    fn size_hint(&self) -> Option<usize> {
        self.inner.size_hint()
    }

    fn snapshot(&self) -> Option<Vec<Seed>> {
        self.inner.snapshot()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;
    use crate::frontier::test_util::{pop_path, seed};
    use crate::frontier::{FrontierExt, InMemFrontier};
    use crate::{Error, ErrorKind};

    /// Fails its first push, as a crash before the seed is queued would.
    #[derive(Default)]
    struct FailsOnce {
        inner: InMemFrontier,
        failed: AtomicBool,
    }

    impl Frontier for FailsOnce {
        async fn push(&self, seed: Seed) -> Result<Admission> {
            if self.failed.swap(true, Ordering::SeqCst) {
                self.inner.push(seed).await
            } else {
                Err(Error::new(ErrorKind::Frontier).with_message("interrupted"))
            }
        }

        async fn pop_with(&self, mode: PopMode) -> Result<NextSeed> {
            self.inner.pop_with(mode).await
        }

        async fn report(&self, report: Report) -> Result<Ack> {
            self.inner.report(report).await
        }
    }

    /// Yields before each push, so pushes started together overlap.
    #[derive(Default)]
    struct Yields<F = InMemFrontier>(F);

    impl<F: Frontier> Frontier for Yields<F> {
        async fn push(&self, seed: Seed) -> Result<Admission> {
            tokio::task::yield_now().await;
            self.0.push(seed).await
        }

        async fn pop_with(&self, mode: PopMode) -> Result<NextSeed> {
            self.0.pop_with(mode).await
        }

        async fn report(&self, report: Report) -> Result<Ack> {
            self.0.report(report).await
        }
    }

    #[tokio::test]
    async fn a_seed_that_was_not_queued_is_not_refused_later() {
        let frontier = Dedup::new(FailsOnce::default(), InMemSeen::default());
        assert!(frontier.push(seed("a")).await.is_err());

        // Found again, e.g. when its page is crawled again after a crash.
        assert_eq!(frontier.push(seed("a")).await.unwrap(), Admission::Queued);
        assert_eq!(pop_path(&frontier).await.as_deref(), Some("a"));
    }

    /// Refuses its first push, as an inner facade such as `max_depth` would.
    #[derive(Default)]
    struct RefusesOnce {
        inner: InMemFrontier,
        refused: AtomicBool,
    }

    impl Frontier for RefusesOnce {
        async fn push(&self, seed: Seed) -> Result<Admission> {
            if self.refused.swap(true, Ordering::SeqCst) {
                self.inner.push(seed).await
            } else {
                Ok(Admission::TooDeep)
            }
        }

        async fn pop_with(&self, mode: PopMode) -> Result<NextSeed> {
            self.inner.pop_with(mode).await
        }

        async fn report(&self, report: Report) -> Result<Ack> {
            self.inner.report(report).await
        }
    }

    #[tokio::test]
    async fn a_seed_refused_further_in_is_not_recorded() {
        let frontier = Dedup::new(RefusesOnce::default(), InMemSeen::default());
        assert_eq!(frontier.push(seed("a")).await.unwrap(), Admission::TooDeep);

        // Found again where the inner facade admits it, e.g. less deep.
        assert_eq!(frontier.push(seed("a")).await.unwrap(), Admission::Queued);
        assert_eq!(
            frontier.push(seed("a")).await.unwrap(),
            Admission::Duplicate
        );
    }

    #[tokio::test]
    async fn the_same_seed_pushed_at_once_is_queued_once() {
        let frontier = Yields::<InMemFrontier>::default().dedup();
        let pushes = (0..8).map(|_| frontier.push(seed("a")));
        let admissions = futures::future::join_all(pushes).await;
        let queued = admissions
            .into_iter()
            .filter(|admission| *admission.as_ref().unwrap() == Admission::Queued)
            .count();
        assert_eq!(queued, 1);
        assert_eq!(pop_path(&frontier).await.as_deref(), Some("a"));
        assert_eq!(pop_path(&frontier).await, None);
    }

    #[tokio::test]
    async fn a_push_waiting_on_one_that_fails_is_queued() {
        let frontier = Dedup::new(Yields(FailsOnce::default()), InMemSeen::default());
        let (first, second) = tokio::join!(frontier.push(seed("a")), frontier.push(seed("a")));
        assert!(first.is_err());
        assert_eq!(second.unwrap(), Admission::Queued);
        assert_eq!(pop_path(&frontier).await.as_deref(), Some("a"));
    }

    #[tokio::test]
    async fn refuses_duplicates() {
        let frontier = InMemFrontier::fifo().dedup();
        assert_eq!(frontier.push(seed("a")).await.unwrap(), Admission::Queued);
        assert_eq!(
            frontier.push(seed("a")).await.unwrap(),
            Admission::Duplicate
        );
        assert_eq!(
            frontier.push(seed("a").with_tag("other")).await.unwrap(),
            Admission::Duplicate
        );
        assert_eq!(frontier.push(seed("b")).await.unwrap(), Admission::Queued);

        assert_eq!(pop_path(&frontier).await.as_deref(), Some("a"));
        assert_eq!(pop_path(&frontier).await.as_deref(), Some("b"));
        assert_eq!(pop_path(&frontier).await, None);
    }
}
