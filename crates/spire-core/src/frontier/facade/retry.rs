use std::future::Future;
use std::time::Duration;

use tokio::time::Instant;

use crate::Result;
use crate::crawler::Attempt;
use crate::frontier::seed::Seed;
use crate::frontier::{Ack, Admission, Frontier, NextSeed, PopMode, Report};

/// How often and how fast a [`Retry`] facade retries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    max_retries: u32,
    base_delay: Duration,
    max_delay: Duration,
    jitter: u32,
}

impl RetryPolicy {
    /// Retries up to `max_retries` times, waiting 500ms before the first
    /// retry and doubling the delay up to 30s, shortened by up to 20% of
    /// jitter.
    #[must_use]
    pub const fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            jitter: 20,
        }
    }

    /// Shortens each delay by up to `percent` of itself, so seeds failing
    /// together are not retried together. Clamped to 100.
    #[must_use]
    pub const fn with_jitter(mut self, percent: u32) -> Self {
        self.jitter = if percent > 100 { 100 } else { percent };
        self
    }

    /// Sets the delay before the first retry.
    #[must_use]
    pub const fn with_base_delay(mut self, delay: Duration) -> Self {
        self.base_delay = delay;
        self
    }

    /// Sets the upper bound of the delay.
    #[must_use]
    pub const fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Returns the delay before trying `attempt`, which is at least 1,
    /// without jitter.
    #[must_use]
    pub fn delay(&self, attempt: Attempt) -> Duration {
        let exponent = attempt.get().saturating_sub(1).min(31);
        self.base_delay
            .saturating_mul(1 << exponent)
            .min(self.max_delay)
    }

    /// Returns the delay before `seed`'s attempt, with jitter derived from
    /// its fingerprint and attempt, so it is stable for a given seed.
    fn jittered(&self, seed: &Seed) -> Duration {
        let delay = self.delay(seed.attempt());
        let mix = seed
            .fingerprint()
            .get()
            .wrapping_add(u64::from(seed.attempt().get()))
            .wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let fraction = u32::try_from((mix >> 40) % 1000).unwrap_or_default();
        let cut = delay * self.jitter / 100 * fraction / 1000;
        delay.saturating_sub(cut)
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::new(3)
    }
}

/// A facade that re-queues seeds reported with [`Report::Retry`].
///
/// The facade decides whether a seed is tried again and when; the storage
/// holds it until then, see [`Report::Requeue`]. A retried seed keeps its
/// priority and is not pushed again, so facades inside this one never refuse
/// it. Once due it joins the queue, and the storage's order decides when it
/// runs. Seeds out of retries, or refused by the storage, are reported inwards
/// as [`Report::Failed`] and refused with [`Ack::RetryRefused`].
#[derive(Debug)]
pub struct Retry<F> {
    inner: F,
    policy: RetryPolicy,
}

impl<F> Retry<F> {
    /// Wraps `inner`, retrying according to `policy`.
    pub const fn new(inner: F, policy: RetryPolicy) -> Self {
        Self { inner, policy }
    }
}

impl<F: Frontier> Frontier for Retry<F> {
    fn push(&self, seed: Seed) -> impl Future<Output = Result<Admission>> + Send {
        self.inner.push(seed)
    }

    fn permits(&self, seed: &Seed) -> bool {
        self.inner.permits(seed)
    }

    fn pop_with(&self, mode: PopMode) -> impl Future<Output = Result<NextSeed>> + Send {
        self.inner.pop_with(mode)
    }

    async fn report(&self, report: Report) -> Result<Ack> {
        let Report::Retry { seed, after } = report else {
            return self.inner.report(report).await;
        };

        if seed.attempt().get() >= self.policy.max_retries {
            tracing::debug!(
                target: crate::TRACING_TARGET,
                uri = %seed.uri(),
                attempt = %seed.attempt(),
                "retries exhausted",
            );
            self.inner.report(Report::Failed(seed)).await?;
            return Ok(Ack::RetryRefused);
        }

        let retried = seed.clone().next_attempt();
        let delay = after.unwrap_or_else(|| self.policy.jittered(&retried));
        let due = Instant::now() + delay;
        let ack = self
            .inner
            .report(Report::Requeue { seed: retried, due })
            .await?;
        if ack == Ack::RetryRefused {
            self.inner.report(Report::Failed(seed)).await?;
        }
        Ok(ack)
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

    use tokio::time::{Instant, advance};

    use super::*;
    use crate::frontier::test_util::{pop_path, seed};
    use crate::frontier::{FrontierExt, InMemFrontier};

    #[test]
    fn policy_delay_doubles_and_caps() {
        let policy = RetryPolicy::new(5)
            .with_base_delay(Duration::from_secs(1))
            .with_max_delay(Duration::from_secs(5))
            .with_jitter(0);
        assert_eq!(policy.delay(Attempt::new(1)), Duration::from_secs(1));
        assert_eq!(policy.delay(Attempt::new(2)), Duration::from_secs(2));
        assert_eq!(policy.delay(Attempt::new(3)), Duration::from_secs(4));
        assert_eq!(policy.delay(Attempt::new(4)), Duration::from_secs(5));
        assert_eq!(policy.delay(Attempt::new(40)), Duration::from_secs(5));
    }

    #[test]
    fn jitter_spreads_and_shortens_delays() {
        let policy = RetryPolicy::new(5).with_base_delay(Duration::from_secs(10));
        let delays: Vec<_> = (0..20)
            .map(|i| policy.jittered(&seed(&i.to_string()).next_attempt()))
            .collect();

        let full = Duration::from_secs(10);
        assert!(
            delays
                .iter()
                .all(|delay| *delay <= full && *delay >= full * 8 / 10)
        );
        assert!(delays.iter().any(|delay| *delay != delays[0]));
        assert_eq!(policy.jittered(&seed("0").next_attempt()), delays[0]);
    }

    #[tokio::test(start_paused = true)]
    async fn retried_seed_waits_for_its_delay() {
        let frontier = InMemFrontier::fifo().retry(1);
        let after = Some(Duration::from_secs(2));
        frontier
            .report(Report::Retry {
                seed: seed("a"),
                after,
            })
            .await
            .unwrap();
        assert_eq!(frontier.size_hint(), Some(1));

        match frontier.pop().await.unwrap() {
            NextSeed::Wait(at) => assert_eq!(at - Instant::now(), Duration::from_secs(2)),
            next => panic!("expected wait, got {next:?}"),
        }

        advance(Duration::from_secs(2)).await;
        let NextSeed::Ready(retried) = frontier.pop().await.unwrap() else {
            panic!("expected the retried seed");
        };
        assert_eq!(retried.attempt(), Attempt::new(1));

        // The single retry is used up.
        frontier
            .report(Report::Retry {
                seed: retried,
                after,
            })
            .await
            .unwrap();
        assert_eq!(pop_path(&frontier).await, None);
        assert!(matches!(frontier.pop().await.unwrap(), NextSeed::Empty));
    }

    #[tokio::test(start_paused = true)]
    async fn queued_seeds_are_served_while_retries_wait() {
        let frontier = InMemFrontier::fifo().retry(3);
        frontier.push(seed("fresh")).await.unwrap();
        let retry = Report::Retry {
            seed: seed("retried"),
            after: Some(Duration::from_secs(1)),
        };
        frontier.report(retry).await.unwrap();

        assert_eq!(pop_path(&frontier).await.as_deref(), Some("fresh"));
        assert!(matches!(frontier.pop().await.unwrap(), NextSeed::Wait(_)));
        advance(Duration::from_secs(1)).await;
        assert_eq!(pop_path(&frontier).await.as_deref(), Some("retried"));
    }

    /// Fails `seed`, to be retried after `secs`.
    async fn retry_after(frontier: &impl Frontier, seed: Seed, secs: u64) {
        let after = Some(Duration::from_secs(secs));
        let ack = frontier.report(Report::Retry { seed, after }).await;
        assert_eq!(ack.unwrap(), Ack::Applied);
    }

    async fn drain(frontier: &impl Frontier) -> Vec<String> {
        let mut order = Vec::new();
        while let Some(path) = pop_path(frontier).await {
            order.push(path);
        }
        order
    }

    #[tokio::test(start_paused = true)]
    async fn a_fresh_seed_of_higher_priority_precedes_a_due_retry() {
        let frontier = InMemFrontier::priority().retry(3);
        retry_after(&frontier, seed("retried").with_priority(1), 1).await;
        frontier
            .push(seed("fresh").with_priority(100))
            .await
            .unwrap();
        advance(Duration::from_secs(1)).await;

        assert_eq!(drain(&frontier).await, ["fresh", "retried"]);
    }

    #[tokio::test(start_paused = true)]
    async fn a_due_retry_of_higher_priority_precedes_a_fresh_seed() {
        let frontier = InMemFrontier::priority().retry(3);
        retry_after(&frontier, seed("retried").with_priority(100), 1).await;
        frontier.push(seed("fresh").with_priority(1)).await.unwrap();
        advance(Duration::from_secs(1)).await;

        let NextSeed::Ready(retried) = frontier.pop().await.unwrap() else {
            panic!("expected the retried seed");
        };
        assert_eq!(retried.priority(), 100);
        assert_eq!(retried.attempt(), Attempt::new(1));
        assert_eq!(drain(&frontier).await, ["fresh"]);
    }

    #[tokio::test(start_paused = true)]
    async fn due_retries_run_by_priority_then_join_equal_seeds_at_the_back() {
        let frontier = InMemFrontier::priority().retry(3);
        retry_after(&frontier, seed("late").with_priority(5), 2).await;
        retry_after(&frontier, seed("high").with_priority(9), 3).await;
        retry_after(&frontier, seed("early").with_priority(5), 1).await;
        frontier
            .push(seed("queued").with_priority(5))
            .await
            .unwrap();
        advance(Duration::from_secs(3)).await;

        assert_eq!(drain(&frontier).await, ["high", "queued", "early", "late"]);
    }

    #[tokio::test(start_paused = true)]
    async fn a_due_retry_joins_a_fifo_at_the_back_and_a_lifo_at_the_top() {
        let fifo = InMemFrontier::fifo().retry(3);
        let lifo = InMemFrontier::lifo().retry(3);
        for path in ["a", "b"] {
            fifo.push(seed(path)).await.unwrap();
            lifo.push(seed(path)).await.unwrap();
        }
        retry_after(&fifo, seed("retried"), 1).await;
        retry_after(&lifo, seed("retried"), 1).await;
        advance(Duration::from_secs(1)).await;

        assert_eq!(drain(&fifo).await, ["a", "b", "retried"]);
        assert_eq!(drain(&lifo).await, ["retried", "b", "a"]);
    }

    #[tokio::test(start_paused = true)]
    async fn a_retry_passes_the_facades_that_admit_seeds() {
        let frontier = InMemFrontier::priority()
            .dedup()
            .filter(|seed| seed.uri().path() != "/retried")
            .max_depth(0)
            .retry(3);
        retry_after(&frontier, seed("retried"), 1).await;
        assert_eq!(frontier.size_hint(), Some(1));
        assert_eq!(frontier.snapshot().map(|seeds| seeds.len()), Some(1));

        advance(Duration::from_secs(1)).await;
        assert_eq!(drain(&frontier).await, ["retried"]);
    }

    #[tokio::test(start_paused = true)]
    async fn the_wait_is_for_the_earliest_retry() {
        let frontier = InMemFrontier::priority().retry(3);
        retry_after(&frontier, seed("later"), 5).await;
        retry_after(&frontier, seed("sooner"), 2).await;

        let NextSeed::Wait(at) = frontier.pop().await.unwrap() else {
            panic!("expected a wait");
        };
        assert_eq!(at - Instant::now(), Duration::from_secs(2));
    }

    #[tokio::test]
    async fn a_seed_storage_refuses_to_requeue_is_reported_failed() {
        /// Storage that leaves `Report::Requeue` to the default.
        #[derive(Default)]
        struct NoRequeue(std::sync::Mutex<Vec<Attempt>>);

        impl Frontier for NoRequeue {
            fn push(&self, _: Seed) -> impl Future<Output = Result<Admission>> + Send {
                std::future::ready(Ok(Admission::Queued))
            }

            fn pop_with(&self, _mode: PopMode) -> impl Future<Output = Result<NextSeed>> + Send {
                std::future::ready(Ok(NextSeed::Empty))
            }

            fn report(&self, report: Report) -> impl Future<Output = Result<Ack>> + Send {
                let ack = match report {
                    Report::Failed(seed) => {
                        self.0.lock().unwrap().push(seed.attempt());
                        Ack::Applied
                    }
                    Report::Requeue { .. } => Ack::RetryRefused,
                    _ => Ack::Applied,
                };
                std::future::ready(Ok(ack))
            }
        }

        let frontier = NoRequeue::default().retry(3);
        let retry = Report::Retry {
            seed: seed("a"),
            after: None,
        };
        assert_eq!(frontier.report(retry).await.unwrap(), Ack::RetryRefused);
        assert_eq!(*frontier.inner.0.lock().unwrap(), [Attempt::FIRST]);
    }
}
