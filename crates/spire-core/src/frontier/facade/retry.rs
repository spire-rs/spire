use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;
use std::fmt;
use std::future::Future;
use std::sync::Mutex;
use std::time::Duration;

use tokio::time::Instant;

use crate::Result;
use crate::frontier::{Ack, Admission, Frontier, Next, Report, Seed};
use crate::request::Attempt;
use crate::sync::lock;

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
/// Retried seeds wait in the facade until their delay passes; they are not
/// pushed again, so facades inside it never refuse them. Waiting seeds are
/// held in memory; they are bounded by the requests that failed, not by the
/// size of the frontier. Seeds out of
/// retries are reported inwards as [`Report::Failed`] and refused with
/// [`Ack::RetryRefused`].
pub struct Retry<F> {
    inner: F,
    policy: RetryPolicy,
    delayed: Mutex<Delayed>,
}

#[derive(Default)]
struct Delayed {
    heap: BinaryHeap<Reverse<Entry>>,
    sequence: u64,
}

struct Entry {
    at: Instant,
    sequence: u64,
    seed: Seed,
}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.at, self.sequence).cmp(&(other.at, other.sequence))
    }
}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for Entry {}

impl Delayed {
    fn insert(&mut self, at: Instant, seed: Seed) {
        self.sequence += 1;
        self.heap.push(Reverse(Entry {
            at,
            sequence: self.sequence,
            seed,
        }));
    }

    fn pop_due(&mut self, now: Instant) -> Option<Seed> {
        if self.earliest()? > now {
            return None;
        }
        self.heap.pop().map(|Reverse(entry)| entry.seed)
    }

    fn earliest(&self) -> Option<Instant> {
        self.heap.peek().map(|Reverse(entry)| entry.at)
    }
}

impl<F> Retry<F> {
    /// Wraps `inner`, retrying according to `policy`.
    pub fn new(inner: F, policy: RetryPolicy) -> Self {
        Self {
            inner,
            policy,
            delayed: Mutex::default(),
        }
    }
}

impl<F: fmt::Debug> fmt::Debug for Retry<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Retry")
            .field("inner", &self.inner)
            .field("policy", &self.policy)
            .field("delayed", &lock(&self.delayed).heap.len())
            .finish()
    }
}

impl<F: Frontier> Frontier for Retry<F> {
    fn push(&self, seed: Seed) -> impl Future<Output = Result<Admission>> + Send {
        self.inner.push(seed)
    }

    async fn pop(&self) -> Result<Next> {
        let now = Instant::now();
        if let Some(seed) = lock(&self.delayed).pop_due(now) {
            return Ok(Next::Ready(seed));
        }

        let next = self.inner.pop().await?;
        let earliest = lock(&self.delayed).earliest();
        Ok(match (next, earliest) {
            (Next::Wait(at), Some(earliest)) => Next::Wait(at.min(earliest)),
            (Next::Empty, Some(earliest)) => Next::Wait(earliest),
            (next, _) => next,
        })
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

        let seed = seed.next_attempt();
        let delay = after.unwrap_or_else(|| self.policy.jittered(&seed));
        lock(&self.delayed).insert(Instant::now() + delay, seed);
        Ok(Ack::Applied)
    }

    fn size_hint(&self) -> Option<usize> {
        let delayed = lock(&self.delayed).heap.len();
        self.inner.size_hint().map(|len| len + delayed)
    }

    fn snapshot(&self) -> Option<Vec<Seed>> {
        let mut seeds = self.inner.snapshot()?;
        let delayed = lock(&self.delayed);
        seeds.extend(delayed.heap.iter().map(|Reverse(entry)| entry.seed.clone()));
        Some(seeds)
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
            Next::Wait(at) => assert_eq!(at - Instant::now(), Duration::from_secs(2)),
            next => panic!("expected wait, got {next:?}"),
        }

        advance(Duration::from_secs(2)).await;
        let Next::Ready(retried) = frontier.pop().await.unwrap() else {
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
        assert!(matches!(frontier.pop().await.unwrap(), Next::Empty));
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
        assert!(matches!(frontier.pop().await.unwrap(), Next::Wait(_)));
        advance(Duration::from_secs(1)).await;
        assert_eq!(pop_path(&frontier).await.as_deref(), Some("retried"));
    }
}
