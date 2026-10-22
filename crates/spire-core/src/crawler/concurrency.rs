//! How many requests one host gets at once: a fixed limit, or one adapted
//! to how the host responds.

use std::num::NonZeroUsize;
use std::time::Duration;

use tokio::time::Instant;

/// How many requests to the same host run at once, see
/// [`Crawler::with_host_concurrency`](crate::Crawler::with_host_concurrency).
///
/// A number is a fixed limit. [`HostConcurrency::adaptive`] gives each host
/// its own limit, adapted to how it responds.
///
/// ```
/// use spire_core::Crawler;
/// use spire_core::backend::Noop;
/// use spire_core::crawler::HostConcurrency;
/// use spire_core::worker::from_fn;
///
/// let worker = from_fn(|_: spire_core::Context<Noop>| async {});
/// let crawler = Crawler::new(Noop, worker).with_host_concurrency(HostConcurrency::adaptive(8));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostConcurrency {
    max: NonZeroUsize,
    adaptive: bool,
}

impl HostConcurrency {
    /// At most `limit` requests to a host at once, at least one.
    #[must_use]
    pub fn fixed(limit: usize) -> Self {
        Self {
            max: NonZeroUsize::new(limit).unwrap_or(NonZeroUsize::MIN),
            adaptive: false,
        }
    }

    /// A limit per host, adapted to how it responds, between one and `max`.
    ///
    /// Each host starts at one request at a time. The limit grows by one
    /// for every round of responses that come back about as fast as the
    /// host's best: one more request after one success at a limit of one,
    /// after two at two, and so on. It halves when the host asks to slow
    /// down, with `429 Too Many Requests` or `503 Service Unavailable`, when
    /// a request times out, or when responses take more than twice the
    /// host's best time, at most once per round. Other failures leave it
    /// alone.
    ///
    /// Response times are how long the fetch took, not the handlers.
    #[must_use]
    pub fn adaptive(max: usize) -> Self {
        Self {
            adaptive: true,
            ..Self::fixed(max)
        }
    }

    /// The most requests a host may get at once.
    pub(crate) const fn max(self) -> usize {
        self.max.get()
    }

    /// Whether each host's limit adapts.
    pub(crate) const fn is_adaptive(self) -> bool {
        self.adaptive
    }
}

impl From<usize> for HostConcurrency {
    fn from(limit: usize) -> Self {
        Self::fixed(limit)
    }
}

/// How a request went, for an adaptive limit.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Feedback {
    /// When the request started.
    pub(crate) started: Instant,
    /// How long its fetch took, if it completed.
    pub(crate) latency: Option<Duration>,
    pub(crate) signal: Signal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Signal {
    /// The request succeeded.
    Ok,
    /// The request failed.
    Failed,
    /// The host asked to slow down, or timed out.
    Throttled,
}

/// Response times up to this many times a host's best are normal.
const SLOW: u32 = 2;

/// A host's best response time follows slower ones by this fraction of the
/// difference, so a host that becomes slower for good is not overloaded
/// forever.
const DRIFT: u32 = 16;

/// A host's adaptive limit: additive increase, multiplicative decrease.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Limit {
    current: usize,
    /// Successes since the limit last grew.
    successes: usize,
    /// The host's best recent response time.
    best: Option<Duration>,
    /// When the limit last halved.
    halved: Option<Instant>,
}

impl Limit {
    pub(crate) const fn new() -> Self {
        Self {
            current: 1,
            successes: 0,
            best: None,
            halved: None,
        }
    }

    /// The requests the host may get at once.
    pub(crate) const fn get(&self) -> usize {
        self.current
    }

    /// Adapts the limit, at most `max`, to how a request went at `now`.
    pub(crate) fn update(&mut self, max: usize, feedback: Feedback, now: Instant) {
        let slow = match (feedback.signal, feedback.latency, self.best) {
            (Signal::Ok, Some(latency), Some(best)) => latency > best * SLOW,
            _ => false,
        };
        if let (Signal::Ok, Some(latency)) = (feedback.signal, feedback.latency) {
            self.best = Some(match self.best {
                Some(best) if latency > best => best + latency.saturating_sub(best) / DRIFT,
                _ => latency,
            });
        }

        if feedback.signal == Signal::Throttled || slow {
            // Requests started before the last halving were sent at the old
            // limit: they halve it once, not once each.
            if self.halved.is_some_and(|at| feedback.started < at) {
                return;
            }
            self.current = (self.current / 2).max(1);
            self.successes = 0;
            self.halved = Some(now);
        } else if feedback.signal == Signal::Ok {
            self.successes += 1;
            if self.successes >= self.current {
                self.current = (self.current + 1).min(max);
                self.successes = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feedback(signal: Signal, started: Instant, latency_ms: u64) -> Feedback {
        Feedback {
            started,
            latency: Some(Duration::from_millis(latency_ms)),
            signal,
        }
    }

    #[test]
    fn grows_by_one_per_round_up_to_the_max() {
        let now = Instant::now();
        let mut limit = Limit::new();
        let mut limits = Vec::new();
        for _ in 0..12 {
            limit.update(4, feedback(Signal::Ok, now, 100), now);
            limits.push(limit.get());
        }
        // One success at 1, two at 2, three at 3, then capped at 4.
        assert_eq!(limits, [2, 2, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4]);
    }

    #[test]
    fn halves_once_per_round_when_throttled_or_slow() {
        let start = Instant::now();
        let mut limit = Limit::new();
        for _ in 0..20 {
            limit.update(8, feedback(Signal::Ok, start, 100), start);
        }
        assert_eq!(limit.get(), 6);

        // A 429 halves it; the other requests of the same round do not.
        let later = start + Duration::from_secs(1);
        limit.update(8, feedback(Signal::Throttled, start, 100), later);
        limit.update(8, feedback(Signal::Throttled, start, 100), later);
        assert_eq!(limit.get(), 3);

        // A request started after it, coming back three times slower than
        // the host's best, halves it again.
        let after = later + Duration::from_millis(1);
        limit.update(8, feedback(Signal::Ok, after, 300), after);
        assert_eq!(limit.get(), 1);
    }

    #[test]
    fn other_failures_leave_it_alone() {
        let now = Instant::now();
        let mut limit = Limit::new();
        limit.update(4, feedback(Signal::Ok, now, 100), now);
        limit.update(4, feedback(Signal::Failed, now, 1), now);
        limit.update(4, feedback(Signal::Failed, now, 10_000), now);
        assert_eq!(limit.get(), 2);
        assert_eq!(limit.best, Some(Duration::from_millis(100)));
    }

    #[test]
    fn the_best_time_follows_slower_hosts_slowly() {
        let now = Instant::now();
        let mut limit = Limit::new();
        limit.update(4, feedback(Signal::Ok, now, 100), now);
        limit.update(4, feedback(Signal::Ok, now, 180), now);
        assert_eq!(limit.best, Some(Duration::from_millis(105)));
        limit.update(4, feedback(Signal::Ok, now, 50), now);
        assert_eq!(limit.best, Some(Duration::from_millis(50)));
    }

    #[test]
    fn limits_are_at_least_one() {
        assert_eq!(HostConcurrency::fixed(0).max(), 1);
        assert_eq!(HostConcurrency::from(3), HostConcurrency::fixed(3));
        assert!(HostConcurrency::adaptive(0).is_adaptive());
    }
}
