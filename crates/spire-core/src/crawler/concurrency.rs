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
    /// alone. At one, a throttled host's gap widens instead, see
    /// [`HostDelay`](crate::crawler::HostDelay); recovering, its gap closes
    /// before its limit grows again.
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
    /// Whether the server answered `304 Not Modified`: quick, with no
    /// body, so not a measure of how fast the host serves pages.
    pub(crate) unchanged: bool,
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
}

impl Limit {
    pub(crate) const fn new() -> Self {
        Self {
            current: 1,
            successes: 0,
            best: None,
        }
    }

    /// The requests the host may get at once.
    pub(crate) const fn get(&self) -> usize {
        self.current
    }

    /// Takes a successful response's time into account, returning whether
    /// it was slow: more than twice the host's best.
    pub(crate) fn observe(&mut self, latency: Option<Duration>) -> bool {
        let Some(latency) = latency else {
            return false;
        };
        let slow = self.best.is_some_and(|best| latency > best * SLOW);
        self.best = Some(match self.best {
            Some(best) if latency > best => best + latency.saturating_sub(best) / DRIFT,
            _ => latency,
        });
        slow
    }

    /// Counts a fast success, growing the limit by one, at most `max`, once
    /// a round of them is in.
    pub(crate) fn grow(&mut self, max: usize) {
        self.successes += 1;
        if self.successes >= self.current {
            self.current = (self.current + 1).min(max);
            self.successes = 0;
        }
    }

    /// Halves the limit, returning `false` if it is already one.
    pub(crate) fn halve(&mut self) -> bool {
        if self.current == 1 {
            return false;
        }
        self.current /= 2;
        self.successes = 0;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn ms(ms: u64) -> Duration {
        Duration::from_millis(ms)
    }

    #[test]
    fn grows_by_one_per_round_up_to_the_max() {
        let mut limit = Limit::new();
        let mut limits = Vec::new();
        for _ in 0..12 {
            limit.grow(4);
            limits.push(limit.get());
        }
        // One success at 1, two at 2, three at 3, then capped at 4.
        assert_eq!(limits, [2, 2, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4]);
    }

    #[test]
    fn halves_down_to_one() {
        let mut limit = Limit::new();
        for _ in 0..20 {
            limit.grow(8);
        }
        assert_eq!(limit.get(), 6);
        assert!(limit.halve());
        assert_eq!(limit.get(), 3);
        assert!(limit.halve());
        assert!(!limit.halve(), "already one");
        assert_eq!(limit.get(), 1);
    }

    #[test]
    fn responses_twice_as_slow_as_the_best_are_slow() {
        let mut limit = Limit::new();
        assert!(!limit.observe(Some(ms(100))));
        assert!(!limit.observe(Some(ms(180))));
        assert_eq!(
            limit.best,
            Some(ms(105)),
            "the best follows slower times slowly"
        );
        assert!(limit.observe(Some(ms(300))));
        assert!(!limit.observe(Some(ms(50))));
        assert_eq!(limit.best, Some(ms(50)));
        assert!(!limit.observe(None));
    }

    #[test]
    fn limits_are_at_least_one() {
        assert_eq!(HostConcurrency::fixed(0).max(), 1);
        assert_eq!(HostConcurrency::from(3), HostConcurrency::fixed(3));
        assert!(HostConcurrency::adaptive(0).is_adaptive());
    }
}
