//! In-memory storage: [`InMemFrontier`], a FIFO, LIFO or priority queue.

use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::fmt;
use std::future::{Future, ready};
use std::sync::Mutex;

use tokio::time::Instant;

use crate::Result;
use crate::frontier::seed::{Fingerprint, Seed};
use crate::frontier::{Ack, Admission, Frontier, Pop, Report, Schedule};
use crate::sync::lock;

/// An in-memory [`Frontier`] that crawls in insertion, reverse or priority
/// order.
pub struct InMemFrontier {
    seeds: Mutex<Store>,
    /// Created on the first schedule, so the constructors stay `const`.
    scheduled: Mutex<Option<Scheduled>>,
}

/// Seeds held back until a time, one per fingerprint, see [`Schedule`].
#[derive(Default)]
struct Scheduled {
    /// Every scheduled seed, with when it is due and the sequence of the
    /// heap entry that is current for it.
    seeds: HashMap<Fingerprint, (Instant, u64, Seed)>,
    /// Due times, earliest first. Entries a later schedule replaced stay
    /// until they come up, and are skipped then.
    due: BinaryHeap<Reverse<(Instant, u64, Fingerprint)>>,
    sequence: u64,
}

impl Scheduled {
    fn insert(&mut self, seed: Seed, due: Instant) {
        self.sequence += 1;
        let fingerprint = seed.fingerprint();
        self.due.push(Reverse((due, self.sequence, fingerprint)));
        self.seeds.insert(fingerprint, (due, self.sequence, seed));
    }

    /// Drops heap entries that a later schedule replaced, so the top is
    /// current.
    fn settle(&mut self) {
        while let Some(Reverse((_, sequence, fingerprint))) = self.due.peek() {
            let current = self
                .seeds
                .get(fingerprint)
                .is_some_and(|(_, current, _)| current == sequence);
            if current {
                break;
            }
            self.due.pop();
        }
    }

    fn pop_due(&mut self, now: Instant) -> Option<Seed> {
        self.settle();
        let Reverse((due, _, fingerprint)) = *self.due.peek()?;
        if due > now {
            return None;
        }
        self.due.pop();
        self.seeds.remove(&fingerprint).map(|(_, _, seed)| seed)
    }

    fn next_due(&mut self) -> Option<Instant> {
        self.settle();
        self.due.peek().map(|Reverse((due, _, _))| *due)
    }
}

enum Store {
    Fifo(VecDeque<Seed>),
    Lifo(VecDeque<Seed>),
    Priority(BinaryHeap<Prioritized>, u64),
}

/// A seed ordered by priority, then by insertion.
struct Prioritized {
    priority: i32,
    sequence: u64,
    seed: Seed,
}

impl Ord for Prioritized {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

impl PartialOrd for Prioritized {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Prioritized {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for Prioritized {}

impl Store {
    fn len(&self) -> usize {
        match self {
            Self::Fifo(seeds) | Self::Lifo(seeds) => seeds.len(),
            Self::Priority(seeds, _) => seeds.len(),
        }
    }

    const fn order(&self) -> &'static str {
        match self {
            Self::Fifo(_) => "fifo",
            Self::Lifo(_) => "lifo",
            Self::Priority(..) => "priority",
        }
    }
}

impl InMemFrontier {
    /// Crawls seeds in the order they were pushed: breadth-first.
    #[must_use]
    pub const fn fifo() -> Self {
        Self::with_store(Store::Fifo(VecDeque::new()))
    }

    /// Crawls the most recently pushed seed first: depth-first.
    #[must_use]
    pub const fn lifo() -> Self {
        Self::with_store(Store::Lifo(VecDeque::new()))
    }

    /// Crawls the seed with the highest [`priority`](Seed::with_priority)
    /// first, and seeds of equal priority in the order they were pushed.
    #[must_use]
    pub const fn priority() -> Self {
        Self::with_store(Store::Priority(BinaryHeap::new(), 0))
    }

    const fn with_store(store: Store) -> Self {
        Self {
            seeds: Mutex::new(store),
            scheduled: Mutex::new(None),
        }
    }

    fn insert(&self, seed: Seed) {
        match &mut *lock(&self.seeds) {
            Store::Fifo(seeds) | Store::Lifo(seeds) => seeds.push_back(seed),
            Store::Priority(seeds, sequence) => {
                *sequence += 1;
                let priority = seed.priority();
                seeds.push(Prioritized {
                    priority,
                    sequence: *sequence,
                    seed,
                });
            }
        }
    }
}

impl Default for InMemFrontier {
    fn default() -> Self {
        Self::fifo()
    }
}

impl fmt::Debug for InMemFrontier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let store = lock(&self.seeds);
        let scheduled = lock(&self.scheduled)
            .as_ref()
            .map_or(0, |scheduled| scheduled.seeds.len());
        f.debug_struct("InMemFrontier")
            .field("order", &store.order())
            .field("len", &store.len())
            .field("scheduled", &scheduled)
            .finish()
    }
}

impl Frontier for InMemFrontier {
    fn push(&self, seed: Seed) -> impl Future<Output = Result<Admission>> + Send {
        self.insert(seed);
        ready(Ok(Admission::Queued))
    }

    /// Hands out a due scheduled seed first, then the queue.
    fn pop(&self) -> impl Future<Output = Result<Pop>> + Send {
        let due = lock(&self.scheduled)
            .as_mut()
            .and_then(|scheduled| scheduled.pop_due(Instant::now()));
        if let Some(seed) = due {
            return ready(Ok(Pop::Ready(seed)));
        }
        let seed = match &mut *lock(&self.seeds) {
            Store::Fifo(seeds) => seeds.pop_front(),
            Store::Lifo(seeds) => seeds.pop_back(),
            Store::Priority(seeds, _) => seeds.pop().map(|prioritized| prioritized.seed),
        };
        ready(Ok(seed.map_or(Pop::Empty, Pop::Ready)))
    }

    /// Re-queues seeds the crawl stopped before finishing, so a
    /// [`snapshot`](Frontier::snapshot) taken after the run includes them.
    fn report(&self, report: Report) -> impl Future<Output = Result<Ack>> + Send {
        let ack = match report {
            Report::Cancelled(seed) => {
                self.insert(seed);
                Ack::Applied
            }
            Report::Retry { .. } => Ack::RetryRefused,
            _ => Ack::Applied,
        };
        ready(Ok(ack))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(lock(&self.seeds).len())
    }

    /// Lists queued and scheduled seeds, the latter without when they are
    /// due.
    fn snapshot(&self) -> Option<Vec<Seed>> {
        let store = lock(&self.seeds);
        let mut seeds: Vec<Seed> = match &*store {
            Store::Fifo(seeds) | Store::Lifo(seeds) => seeds.iter().cloned().collect(),
            Store::Priority(seeds, _) => seeds.iter().map(|entry| entry.seed.clone()).collect(),
        };
        if let Some(scheduled) = &*lock(&self.scheduled) {
            seeds.extend(scheduled.seeds.values().map(|(_, _, seed)| seed.clone()));
        }
        Some(seeds)
    }
}

impl Schedule for InMemFrontier {
    fn schedule(&self, seed: Seed, due: Instant) -> impl Future<Output = Result<()>> + Send {
        lock(&self.scheduled)
            .get_or_insert_with(Scheduled::default)
            .insert(seed, due);
        ready(Ok(()))
    }

    fn next_due(&self) -> impl Future<Output = Result<Option<Instant>>> + Send {
        ready(Ok(lock(&self.scheduled)
            .as_mut()
            .and_then(Scheduled::next_due)))
    }
}

impl Extend<Seed> for InMemFrontier {
    /// Queues seeds as is, e.g. from a [`snapshot`](Frontier::snapshot),
    /// without any facade refusing them.
    fn extend<I: IntoIterator<Item = Seed>>(&mut self, seeds: I) {
        for seed in seeds {
            self.insert(seed);
        }
    }
}

impl FromIterator<Seed> for InMemFrontier {
    /// Collects seeds into a FIFO frontier, e.g. to restore a snapshot.
    fn from_iter<I: IntoIterator<Item = Seed>>(seeds: I) -> Self {
        let mut frontier = Self::fifo();
        frontier.extend(seeds);
        frontier
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontier::test_util::{pop_path, seed};

    #[tokio::test]
    async fn fifo_and_lifo_order() {
        let fifo = InMemFrontier::fifo();
        let lifo = InMemFrontier::lifo();
        for path in ["a", "b"] {
            assert!(fifo.push(seed(path)).await.unwrap().is_queued());
            assert!(lifo.push(seed(path)).await.unwrap().is_queued());
        }
        assert_eq!(fifo.size_hint(), Some(2));

        assert_eq!(pop_path(&fifo).await.as_deref(), Some("a"));
        assert_eq!(pop_path(&lifo).await.as_deref(), Some("b"));
        assert_eq!(pop_path(&fifo).await.as_deref(), Some("b"));
        assert!(matches!(fifo.pop().await.unwrap(), Pop::Empty));
    }

    #[tokio::test]
    async fn priority_order() {
        let frontier = InMemFrontier::priority();
        for (path, priority) in [("low", -1), ("first", 0), ("high", 5), ("second", 0)] {
            frontier
                .push(seed(path).with_priority(priority))
                .await
                .unwrap();
        }
        let mut order = Vec::new();
        while let Some(path) = pop_path(&frontier).await {
            order.push(path);
        }
        assert_eq!(order, ["high", "first", "second", "low"]);
    }
}
