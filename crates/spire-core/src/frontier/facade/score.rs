use std::fmt;
use std::future::Future;

use crate::Result;
use crate::frontier::score::Score;
use crate::frontier::seed::Seed;
use crate::frontier::{Ack, Admission, Frontier, NextSeed, Report};

/// A facade that adds a score to the priority of every seed pushed, see
/// [`FrontierExt::score`](crate::frontier::FrontierExt::score).
pub struct Scored<F, S> {
    inner: F,
    scorer: S,
}

impl<F, S> Scored<F, S> {
    /// Wraps `inner`, adding `scorer`'s score to every pushed seed.
    pub const fn new(inner: F, scorer: S) -> Self {
        Self { inner, scorer }
    }
}

impl<F: fmt::Debug, S> fmt::Debug for Scored<F, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Scored")
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

impl<F: Frontier, S: Score> Frontier for Scored<F, S> {
    fn push(&self, seed: Seed) -> impl Future<Output = Result<Admission>> + Send {
        let priority = seed.priority().saturating_add(self.scorer.score(&seed));
        self.inner.push(seed.with_priority(priority))
    }

    fn pop(&self) -> impl Future<Output = Result<NextSeed>> + Send {
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
    use crate::crawler::Depth;
    use crate::frontier::score::{Keywords, ShallowFirst};
    use crate::frontier::seed::Seed;
    use crate::frontier::test_util::{pop_path, seed};
    use crate::frontier::{Frontier, FrontierExt, InMemFrontier};

    #[tokio::test]
    async fn crawls_in_score_order() {
        let frontier = InMemFrontier::priority().score(Keywords::new(["laptop", "review"]));
        for path in ["about", "laptop", "laptop-review", "contact"] {
            frontier.push(seed(path)).await.unwrap();
        }
        let order = [
            pop_path(&frontier).await,
            pop_path(&frontier).await,
            pop_path(&frontier).await,
            pop_path(&frontier).await,
        ];
        assert_eq!(
            order.map(Option::unwrap),
            ["laptop-review", "laptop", "about", "contact"]
        );
    }

    #[tokio::test]
    async fn scores_add_to_the_priority_and_to_each_other() {
        let frontier = InMemFrontier::priority()
            .score(|_: &Seed| 10)
            .score(ShallowFirst);
        let deep = seed("deep").with_depth(Depth::ROOT.child().child());
        frontier.push(deep.with_priority(5)).await.unwrap();
        frontier
            .push(seed("max").with_priority(i32::MAX))
            .await
            .unwrap();

        let mut priorities: Vec<i32> = frontier
            .snapshot()
            .unwrap()
            .iter()
            .map(Seed::priority)
            .collect();
        priorities.sort_unstable();
        assert_eq!(priorities, [13, i32::MAX]);
    }
}
