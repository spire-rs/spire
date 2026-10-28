use std::fmt;
use std::future::Future;
use std::marker::PhantomData;

use tokio::time::Instant;

use crate::Result;
use crate::crawler::Revisit;
use crate::frontier::seed::Seed;
use crate::frontier::{Ack, Admission, Frontier, NextSeed, Report, Schedule};

/// Crawls pages again on a schedule, see
/// [`FrontierExt::revisit`](crate::frontier::FrontierExt::revisit).
///
/// A seed reported done or failed whose tag is a `T` with an
/// [`interval`](Revisit::interval) is scheduled on the storage to be crawled
/// again that long after, on its first attempt; a failed page may come back.
/// Skipped seeds, e.g. disallowed by robots.txt, are not. Once nothing else
/// is queued, the frontier waits for the next revisit, so the crawl keeps
/// going until it is stopped.
///
/// The storage keeps the schedule, one revisit per page: in memory, or in a
/// redb file, where it survives a restart. Revisits come out of the storage
/// like queued seeds, so facades above it apply to them too, and
/// deduplication does not refuse them: they are never pushed.
///
/// A [snapshot](Frontier::snapshot) of in-memory storage lists scheduled
/// seeds without when they are due, so a crawl resumed from it crawls them
/// at once; keep revisits in redb to keep their timing.
pub struct Revisits<F, T> {
    inner: F,
    marker: PhantomData<fn() -> T>,
}

impl<F, T> Revisits<F, T> {
    /// Wraps `inner`, revisiting by `T`'s intervals.
    pub const fn new(inner: F) -> Self {
        Self {
            inner,
            marker: PhantomData,
        }
    }
}

impl<F: fmt::Debug, T> fmt::Debug for Revisits<F, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Revisits")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<F: Schedule, T: Revisit> Frontier for Revisits<F, T> {
    fn push(&self, seed: Seed) -> impl Future<Output = Result<Admission>> + Send {
        self.inner.push(seed)
    }

    async fn pop(&self) -> Result<NextSeed> {
        match self.inner.pop().await? {
            NextSeed::Empty => Ok(self
                .inner
                .next_due()
                .await?
                .map_or(NextSeed::Empty, NextSeed::Wait)),
            next => Ok(next),
        }
    }

    async fn report(&self, report: Report) -> Result<Ack> {
        let (Report::Done(seed) | Report::Failed(seed)) = &report else {
            return self.inner.report(report).await;
        };
        let interval = seed.tag().and_then(T::from_tag).and_then(Revisit::interval);
        if let Some(interval) = interval {
            // Scheduled before the seed is reported: a crash in between
            // crawls it once more, rather than losing its revisit, and a
            // page has one revisit however often it is scheduled.
            tracing::debug!(
                target: crate::TRACING_TARGET,
                uri = %seed.uri(),
                in_secs = interval.as_secs(),
                "scheduling a revisit",
            );
            let revisit = seed.clone().first_attempt();
            self.inner
                .schedule(revisit, Instant::now() + interval)
                .await?;
        }
        self.inner.report(report).await
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
    use std::time::Duration;

    use tokio::time::advance;

    use super::*;
    use crate::crawler::{Tag, Tags};
    use crate::frontier::test_util::{pop_path, seed};
    use crate::frontier::{FrontierExt, InMemFrontier};

    #[derive(Debug, Clone, Copy)]
    enum Page {
        Listing,
        About,
    }

    impl From<Page> for Tag {
        fn from(page: Page) -> Self {
            Self::from_static(match page {
                Page::Listing => "listing",
                Page::About => "about",
            })
        }
    }

    impl Tags for Page {
        const ALL: &'static [Self] = &[Self::Listing, Self::About];
    }

    impl Revisit for Page {
        fn interval(self) -> Option<Duration> {
            match self {
                Self::Listing => Some(Duration::from_mins(1)),
                Self::About => None,
            }
        }
    }

    #[tokio::test(start_paused = true)]
    async fn done_and_failed_seeds_with_an_interval_come_back_when_due() {
        let frontier = InMemFrontier::fifo().revisit::<Page>().dedup();
        let listing = seed("list").with_tag(Page::Listing);
        frontier.push(listing.clone()).await.unwrap();
        assert_eq!(pop_path(&frontier).await.as_deref(), Some("list"));

        frontier
            .report(Report::Done(listing.next_attempt()))
            .await
            .unwrap();
        frontier
            .report(Report::Done(seed("about").with_tag(Page::About)))
            .await
            .unwrap();
        frontier
            .report(Report::Done(seed("untagged")))
            .await
            .unwrap();
        frontier
            .report(Report::Skipped(seed("blocked").with_tag(Page::Listing)))
            .await
            .unwrap();

        // Only the listing is scheduled: nothing is ready, so the frontier
        // waits for it.
        assert_eq!(frontier.size_hint(), Some(0));
        let NextSeed::Wait(at) = frontier.pop().await.unwrap() else {
            panic!("expected a wait for the revisit");
        };
        assert_eq!(at - Instant::now(), Duration::from_mins(1));

        // Due, it is handed out on its first attempt again, past the
        // deduplication that saw it already.
        advance(Duration::from_mins(1)).await;
        let NextSeed::Ready(revisit) = frontier.pop().await.unwrap() else {
            panic!("expected the revisit");
        };
        assert_eq!(revisit.uri().path(), "/list");
        assert_eq!(revisit.attempt(), crate::crawler::Attempt::FIRST);

        // A failure schedules it again too: the page may come back.
        frontier.report(Report::Failed(revisit)).await.unwrap();
        assert!(matches!(frontier.pop().await.unwrap(), NextSeed::Wait(_)));
    }

    #[tokio::test(start_paused = true)]
    async fn a_page_has_one_revisit() {
        let storage = std::sync::Arc::new(InMemFrontier::fifo());
        let frontier = Revisits::<_, Page>::new(std::sync::Arc::clone(&storage));
        for _ in 0..3 {
            let done = Report::Done(seed("list").with_tag(Page::Listing));
            frontier.report(done).await.unwrap();
            advance(Duration::from_secs(1)).await;
        }
        assert_eq!(storage.snapshot().map(|seeds| seeds.len()), Some(1));

        // The last schedule wins: reported at 2 s, it is due at 62 s, so
        // still waiting at 61 s.
        advance(Duration::from_secs(58)).await;
        assert!(matches!(frontier.pop().await.unwrap(), NextSeed::Wait(_)));
        advance(Duration::from_secs(1)).await;
        assert!(matches!(frontier.pop().await.unwrap(), NextSeed::Ready(_)));
        assert!(matches!(frontier.pop().await.unwrap(), NextSeed::Empty));
    }

    #[tokio::test(start_paused = true)]
    async fn storage_alone_does_not_wait_for_revisits() {
        let storage = InMemFrontier::fifo();
        let due = Instant::now() + Duration::from_mins(1);
        storage.schedule(seed("later"), due).await.unwrap();
        storage.push(seed("now")).await.unwrap();

        // Without `Revisits`, a crawl over it ends once nothing is queued.
        assert_eq!(pop_path(&storage).await.as_deref(), Some("now"));
        assert!(matches!(storage.pop().await.unwrap(), NextSeed::Empty));
        assert_eq!(storage.next_due().await.unwrap(), Some(due));
    }
}
