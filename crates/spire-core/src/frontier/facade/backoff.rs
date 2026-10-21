use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::future::Future;
use std::sync::Mutex;

use tokio::time::Instant;

use crate::Result;
use crate::frontier::seed::Seed;
use crate::frontier::{Ack, Admission, Frontier, Next, Report};
use crate::request::Tag;
use crate::sync::lock;

/// A facade that holds back seeds of tags paused by [`Report::Backoff`].
///
/// Seeds of a paused tag that come out of the inner frontier are parked in
/// the facade and released, in order, once the pause ends. Seeds of other
/// tags are crawled meanwhile.
pub struct TagBackoff<F> {
    inner: F,
    state: Mutex<State>,
    max_parked: usize,
}

#[derive(Default)]
struct State {
    /// Pause deadlines; untagged seeds are keyed by `None`.
    paused: HashMap<Option<Tag>, Instant>,
    /// Parked seeds; queues are never empty.
    parked: HashMap<Option<Tag>, VecDeque<Seed>>,
}

impl State {
    fn is_paused(&self, tag: Option<&Tag>, now: Instant) -> bool {
        let tag = tag.cloned();
        self.paused.get(&tag).is_some_and(|until| *until > now)
    }

    /// Takes a parked seed whose pause has ended.
    fn release(&mut self, now: Instant) -> Option<Seed> {
        self.paused.retain(|_, until| *until > now);
        let tag = self
            .parked
            .keys()
            .find(|tag| !self.paused.contains_key(*tag))?
            .clone();

        let queue = self.parked.get_mut(&tag)?;
        let seed = queue.pop_front();
        if queue.is_empty() {
            self.parked.remove(&tag);
        }
        seed
    }

    fn park(&mut self, seed: Seed) {
        let tag = seed.tag().cloned();
        self.parked.entry(tag).or_default().push_back(seed);
    }

    /// The earliest instant at which a parked seed is released.
    fn earliest(&self) -> Option<Instant> {
        self.parked
            .keys()
            .filter_map(|tag| self.paused.get(tag))
            .min()
            .copied()
    }

    fn parked_len(&self) -> usize {
        self.parked.values().map(VecDeque::len).sum()
    }
}

impl<F> TagBackoff<F> {
    /// Wraps `inner`, parking at most 10 000 seeds.
    pub fn new(inner: F) -> Self {
        Self {
            inner,
            state: Mutex::default(),
            max_parked: 10_000,
        }
    }

    /// Parks at most `max` seeds. Once full, the inner frontier is not read
    /// until a pause ends, so a long pause cannot pull a whole persistent
    /// frontier into memory.
    #[must_use]
    pub fn with_max_parked(mut self, max: usize) -> Self {
        self.max_parked = max.max(1);
        self
    }
}

impl<F: fmt::Debug> fmt::Debug for TagBackoff<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = lock(&self.state);
        f.debug_struct("TagBackoff")
            .field("inner", &self.inner)
            .field("paused", &state.paused.len())
            .field("parked", &state.parked_len())
            .field("max_parked", &self.max_parked)
            .finish()
    }
}

impl<F: Frontier> Frontier for TagBackoff<F> {
    fn push(&self, seed: Seed) -> impl Future<Output = Result<Admission>> + Send {
        self.inner.push(seed)
    }

    async fn pop(&self) -> Result<Next> {
        let now = Instant::now();
        if let Some(seed) = lock(&self.state).release(now) {
            return Ok(Next::Ready(seed));
        }

        loop {
            {
                let state = lock(&self.state);
                if state.parked_len() >= self.max_parked
                    && let Some(earliest) = state.earliest()
                {
                    return Ok(Next::Wait(earliest));
                }
            }

            let next = self.inner.pop().await?;
            let mut state = lock(&self.state);
            return Ok(match next {
                Next::Ready(seed) if state.is_paused(seed.tag(), now) => {
                    state.park(seed);
                    continue;
                }
                Next::Ready(seed) => Next::Ready(seed),
                Next::Wait(at) => {
                    Next::Wait(state.earliest().map_or(at, |earliest| at.min(earliest)))
                }
                Next::Empty => state.earliest().map_or(Next::Empty, Next::Wait),
            });
        }
    }

    async fn report(&self, report: Report) -> Result<Ack> {
        let Report::Backoff { tag, until } = report else {
            return self.inner.report(report).await;
        };

        let mut state = lock(&self.state);
        let deadline = state.paused.entry(tag).or_insert(until);
        *deadline = (*deadline).max(until);
        Ok(Ack::Applied)
    }

    fn size_hint(&self) -> Option<usize> {
        let parked = lock(&self.state).parked_len();
        self.inner.size_hint().map(|len| len + parked)
    }

    fn snapshot(&self) -> Option<Vec<Seed>> {
        let mut seeds = self.inner.snapshot()?;
        let state = lock(&self.state);
        seeds.extend(state.parked.values().flatten().cloned());
        Some(seeds)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::advance;

    use super::*;
    use crate::frontier::FrontierExt;
    use crate::frontier::memory::InMemFrontier;
    use crate::frontier::test_util::{pop_path, seed};

    fn backoff(tag: Option<&str>, secs: u64) -> Report {
        Report::Backoff {
            tag: tag.map(Tag::from),
            until: Instant::now() + Duration::from_secs(secs),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn paused_tag_is_parked_while_others_run() {
        let frontier = InMemFrontier::fifo().tag_backoff();
        frontier
            .push(seed("slow-1").with_tag("slow"))
            .await
            .unwrap();
        frontier.push(seed("fast").with_tag("fast")).await.unwrap();
        frontier
            .push(seed("slow-2").with_tag("slow"))
            .await
            .unwrap();
        frontier.report(backoff(Some("slow"), 5)).await.unwrap();

        assert_eq!(pop_path(&frontier).await.as_deref(), Some("fast"));
        assert_eq!(frontier.size_hint(), Some(2));
        match frontier.pop().await.unwrap() {
            Next::Wait(at) => assert_eq!(at - Instant::now(), Duration::from_secs(5)),
            next => panic!("expected wait, got {next:?}"),
        }

        advance(Duration::from_secs(5)).await;
        assert_eq!(pop_path(&frontier).await.as_deref(), Some("slow-1"));
        assert_eq!(pop_path(&frontier).await.as_deref(), Some("slow-2"));
        assert!(matches!(frontier.pop().await.unwrap(), Next::Empty));
    }

    #[tokio::test(start_paused = true)]
    async fn stops_reading_when_full() {
        let frontier = InMemFrontier::fifo().tag_backoff().with_max_parked(1);
        for path in ["a", "b", "free"] {
            frontier
                .push(seed(path).with_tag(if path == "free" { "free" } else { "slow" }))
                .await
                .unwrap();
        }
        frontier.report(backoff(Some("slow"), 5)).await.unwrap();

        // One parked seed fills the facade, so the free seed waits too.
        assert!(matches!(frontier.pop().await.unwrap(), Next::Wait(_)));
        assert_eq!(frontier.size_hint(), Some(3));
        advance(Duration::from_secs(5)).await;
        assert_eq!(pop_path(&frontier).await.as_deref(), Some("a"));
    }

    #[tokio::test(start_paused = true)]
    async fn untagged_seeds_have_their_own_scope() {
        let frontier = InMemFrontier::fifo().tag_backoff();
        frontier.push(seed("untagged")).await.unwrap();
        frontier.push(seed("tagged").with_tag("a")).await.unwrap();
        frontier.report(backoff(None, 1)).await.unwrap();

        assert_eq!(pop_path(&frontier).await.as_deref(), Some("tagged"));
        advance(Duration::from_secs(1)).await;
        assert_eq!(pop_path(&frontier).await.as_deref(), Some("untagged"));
    }

    #[tokio::test(start_paused = true)]
    async fn longer_pause_wins() {
        let frontier = InMemFrontier::fifo().tag_backoff();
        frontier.push(seed("a").with_tag("a")).await.unwrap();
        frontier.report(backoff(Some("a"), 10)).await.unwrap();
        frontier.report(backoff(Some("a"), 1)).await.unwrap();

        advance(Duration::from_secs(1)).await;
        assert!(matches!(frontier.pop().await.unwrap(), Next::Wait(_)));
        advance(Duration::from_secs(9)).await;
        assert_eq!(pop_path(&frontier).await.as_deref(), Some("a"));
    }
}
