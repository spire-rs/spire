use std::cmp::Ordering;
use std::collections::{BinaryHeap, VecDeque};
use std::fmt;
use std::future::{Future, ready};
use std::sync::Mutex;

use crate::Result;
use crate::frontier::{Ack, Admission, Frontier, Next, Report, Seed};
use crate::sync::lock;

/// An in-memory [`Frontier`] that crawls in insertion, reverse or priority
/// order.
pub struct InMemFrontier {
    seeds: Mutex<Store>,
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
        f.debug_struct("InMemFrontier")
            .field("order", &store.order())
            .field("len", &store.len())
            .finish()
    }
}

impl Frontier for InMemFrontier {
    fn push(&self, seed: Seed) -> impl Future<Output = Result<Admission>> + Send {
        self.insert(seed);
        ready(Ok(Admission::Queued))
    }

    fn pop(&self) -> impl Future<Output = Result<Next>> + Send {
        let seed = match &mut *lock(&self.seeds) {
            Store::Fifo(seeds) => seeds.pop_front(),
            Store::Lifo(seeds) => seeds.pop_back(),
            Store::Priority(seeds, _) => seeds.pop().map(|prioritized| prioritized.seed),
        };
        ready(Ok(seed.map_or(Next::Empty, Next::Ready)))
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

    fn snapshot(&self) -> Option<Vec<Seed>> {
        let store = lock(&self.seeds);
        let seeds = match &*store {
            Store::Fifo(seeds) | Store::Lifo(seeds) => seeds.iter().cloned().collect(),
            Store::Priority(seeds, _) => seeds.iter().map(|entry| entry.seed.clone()).collect(),
        };
        Some(seeds)
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
        assert!(matches!(fifo.pop().await.unwrap(), Next::Empty));
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
