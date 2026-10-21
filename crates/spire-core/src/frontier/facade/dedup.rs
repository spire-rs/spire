use std::fmt;
use std::future::Future;

use crate::Result;
use crate::frontier::seed::Seed;
use crate::frontier::seen::{InMemSeen, SeenSet};
use crate::frontier::{Ack, Admission, Frontier, Next, Report};

/// A facade that refuses seeds seen before, see [`Seed::fingerprint`].
///
/// Retried seeds are not pushed again, so they are never refused.
pub struct Dedup<F, S = InMemSeen> {
    inner: F,
    seen: S,
}

impl<F, S> Dedup<F, S> {
    /// Wraps `inner`, remembering seeds in `seen`.
    pub const fn new(inner: F, seen: S) -> Self {
        Self { inner, seen }
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
        if self.seen.insert(seed.fingerprint()).await? {
            self.inner.push(seed).await
        } else {
            Ok(Admission::Duplicate)
        }
    }

    fn pop(&self) -> impl Future<Output = Result<Next>> + Send {
        self.inner.pop()
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
    use crate::frontier::memory::InMemFrontier;
    use crate::frontier::test_util::{pop_path, seed};
    use crate::frontier::{Admission, Frontier, FrontierExt};

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
