//! Delays between the requests to one host: fixed, or adapted to how fast
//! the host answers.

use std::time::Duration;

/// How long to wait between starting two requests to the same host, see
/// [`Crawler::with_host_delay`](crate::Crawler::with_host_delay).
///
/// A [`Duration`] is a fixed delay. [`HostDelay::adaptive`] adapts the delay
/// of each host to how fast it answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostDelay {
    /// The same delay for every host.
    Fixed(Duration),
    /// A delay per host, adapted to its responses.
    Adaptive(AdaptiveDelay),
}

impl HostDelay {
    /// Adapts each host's delay to how fast it answers, see
    /// [`AdaptiveDelay`].
    #[must_use]
    pub const fn adaptive() -> AdaptiveDelay {
        AdaptiveDelay::new()
    }

    /// The delay before a host's first request.
    pub(crate) fn initial(self) -> Duration {
        match self {
            Self::Fixed(delay) => delay,
            Self::Adaptive(adaptive) => adaptive.start(),
        }
    }

    /// Whether every host's delay is always zero.
    pub(crate) fn is_zero(self) -> bool {
        matches!(self, Self::Fixed(delay) if delay.is_zero())
    }
}

impl Default for HostDelay {
    fn default() -> Self {
        Self::Fixed(Duration::ZERO)
    }
}

impl From<Duration> for HostDelay {
    fn from(delay: Duration) -> Self {
        Self::Fixed(delay)
    }
}

impl From<AdaptiveDelay> for HostDelay {
    fn from(adaptive: AdaptiveDelay) -> Self {
        Self::Adaptive(adaptive)
    }
}

/// A delay per host, adapted to how fast it answers.
///
/// After every response, a host's delay moves halfway towards its response
/// time divided by the [target concurrency](Self::with_target_concurrency):
/// a host that slows down gets fewer requests, and a fast one more. A
/// response that failed never lowers the delay, so a server failing fast does
/// not look fast. A host that asks to slow down, with `429 Too Many
/// Requests` or `503 Service Unavailable`, or that times out, doubles it.
///
/// The response time is how long the fetch took, not the handlers. Delays
/// stay [within bounds](Self::with_bounds), and never go below a host's
/// robots.txt `Crawl-delay`.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
///
/// use spire_core::Crawler;
/// use spire_core::backend::Noop;
/// use spire_core::crawler::HostDelay;
/// use spire_core::worker::from_fn;
///
/// let delay = HostDelay::adaptive()
///     .with_target_concurrency(2)
///     .with_bounds(Duration::from_millis(100), Duration::from_secs(30));
/// let crawler = Crawler::new(Noop, from_fn(|_: spire_core::Context<Noop>| async {}))
///     .with_host_delay(delay);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdaptiveDelay {
    target_concurrency: u32,
    start: Duration,
    min: Duration,
    max: Duration,
}

impl AdaptiveDelay {
    /// Targets one request in flight per host, starting each host at 1s,
    /// between no delay and 60s.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            target_concurrency: 1,
            start: Duration::from_secs(1),
            min: Duration::ZERO,
            max: Duration::from_mins(1),
        }
    }

    /// Aims for `concurrency` requests in flight per host on average, at
    /// least one: the delay a host settles on is its response time divided
    /// by it. Higher is faster and harder on the host.
    /// `with_host_concurrency` still caps the requests actually in flight.
    #[must_use]
    pub fn with_target_concurrency(mut self, concurrency: usize) -> Self {
        self.target_concurrency = u32::try_from(concurrency.max(1)).unwrap_or(u32::MAX);
        self
    }

    /// Sets the delay before a host's first request, clamped to the bounds.
    #[must_use]
    pub const fn with_start(mut self, delay: Duration) -> Self {
        self.start = delay;
        self
    }

    /// Keeps every delay between `min` and `max`, swapped if `max` is
    /// smaller.
    #[must_use]
    pub const fn with_bounds(mut self, min: Duration, max: Duration) -> Self {
        if min.as_nanos() <= max.as_nanos() {
            self.min = min;
            self.max = max;
        } else {
            self.min = max;
            self.max = min;
        }
        self
    }

    /// Returns the delay after `current`, given how the last request went.
    pub(crate) fn next(&self, current: Duration, feedback: Feedback) -> Duration {
        let next = match (feedback.signal, feedback.latency) {
            (Signal::Throttled, _) => (current * 2).max(self.start),
            (signal, Some(latency)) => {
                let target = latency / self.target_concurrency;
                let average = (current + target) / 2;
                match signal {
                    Signal::Failed => average.max(current),
                    Signal::Ok | Signal::Throttled => average,
                }
            }
            (_, None) => current,
        };
        next.clamp(self.min, self.max)
    }

    /// The delay before a host's first request.
    pub(crate) fn start(&self) -> Duration {
        self.start.clamp(self.min, self.max)
    }
}

impl Default for AdaptiveDelay {
    fn default() -> Self {
        Self::new()
    }
}

/// How a request went, for an [`AdaptiveDelay`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct Feedback {
    /// How long the fetch took, if it completed.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn feedback(signal: Signal, latency_ms: u64) -> Feedback {
        Feedback {
            latency: Some(Duration::from_millis(latency_ms)),
            signal,
        }
    }

    #[test]
    fn moves_halfway_towards_the_latency_over_the_target() {
        let delay = AdaptiveDelay::new().with_target_concurrency(2);
        let current = Duration::from_secs(1);
        // 400ms over a target of 2 is 200ms; halfway from 1000ms is 600ms.
        let next = delay.next(current, feedback(Signal::Ok, 400));
        assert_eq!(next, Duration::from_millis(600));
        let next = delay.next(Duration::from_millis(100), feedback(Signal::Ok, 900));
        assert_eq!(next, Duration::from_millis(275));
    }

    #[test]
    fn failures_never_lower_the_delay() {
        let delay = AdaptiveDelay::new();
        let current = Duration::from_secs(1);
        assert_eq!(delay.next(current, feedback(Signal::Failed, 10)), current);
        let slower = delay.next(current, feedback(Signal::Failed, 3000));
        assert_eq!(slower, Duration::from_secs(2));
    }

    #[test]
    fn throttling_doubles_the_delay_within_bounds() {
        let delay = AdaptiveDelay::new().with_bounds(Duration::ZERO, Duration::from_secs(5));
        let throttled = Feedback {
            latency: None,
            signal: Signal::Throttled,
        };
        assert_eq!(
            delay.next(Duration::from_secs(2), throttled),
            Duration::from_secs(4)
        );
        assert_eq!(
            delay.next(Duration::from_secs(4), throttled),
            Duration::from_secs(5)
        );
        // From no delay at all, the start delay.
        assert_eq!(
            delay.next(Duration::ZERO, throttled),
            Duration::from_secs(1)
        );
    }

    #[test]
    fn bounds_and_missing_latencies() {
        let delay = AdaptiveDelay::new()
            .with_bounds(Duration::from_secs(1), Duration::from_millis(500))
            .with_start(Duration::ZERO);
        assert_eq!(delay.start(), Duration::from_millis(500));
        let next = delay.next(Duration::from_millis(700), feedback(Signal::Ok, 0));
        assert_eq!(next, Duration::from_millis(500));
        let none = Feedback {
            latency: None,
            signal: Signal::Ok,
        };
        assert_eq!(
            delay.next(Duration::from_millis(700), none),
            Duration::from_millis(700)
        );
    }

    #[test]
    fn target_concurrency_is_at_least_one() {
        let delay = AdaptiveDelay::new().with_target_concurrency(0);
        assert_eq!(delay, AdaptiveDelay::new());
    }
}
