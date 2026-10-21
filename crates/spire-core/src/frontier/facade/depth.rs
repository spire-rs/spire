use std::future::Future;

use crate::Result;
use crate::frontier::{Ack, Admission, Frontier, Next, Report, Seed};
use crate::request::Depth;

/// A facade that refuses seeds deeper than a limit.
#[derive(Debug)]
pub struct MaxDepth<F> {
    inner: F,
    max: Depth,
}

impl<F> MaxDepth<F> {
    /// Wraps `inner`, refusing seeds deeper than `max`.
    pub const fn new(inner: F, max: u32) -> Self {
        Self {
            inner,
            max: Depth::new(max),
        }
    }
}

impl<F: Frontier> Frontier for MaxDepth<F> {
    async fn push(&self, seed: Seed) -> Result<Admission> {
        if seed.depth() > self.max {
            return Ok(Admission::TooDeep);
        }
        self.inner.push(seed).await
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
    use crate::frontier::test_util::seed;
    use crate::frontier::{Admission, Frontier, FrontierExt, InMemFrontier};
    use crate::request::Depth;

    #[tokio::test]
    async fn refuses_deep_seeds() {
        let frontier = InMemFrontier::fifo().max_depth(1);
        let deep = |depth| seed("a").with_depth(Depth::new(depth));
        assert_eq!(frontier.push(deep(0)).await.unwrap(), Admission::Queued);
        assert_eq!(frontier.push(deep(1)).await.unwrap(), Admission::Queued);
        assert_eq!(frontier.push(deep(2)).await.unwrap(), Admission::TooDeep);
        assert_eq!(frontier.size_hint(), Some(2));
    }
}
