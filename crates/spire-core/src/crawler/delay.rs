//! The gap between the requests to one host.

use std::ops::RangeInclusive;
use std::time::Duration;

/// The gap between starting two requests to the same host, see
/// [`Crawler::with_host_delay`](crate::Crawler::with_host_delay): at least
/// a minimum, growing towards a maximum while the host is throttled.
///
/// A host is throttled when it answers `429 Too Many Requests` or `503
/// Service Unavailable`, or a request to it times out.
/// Once its concurrency cannot go lower, at a fixed limit or at one with
/// [`HostConcurrency::adaptive`](crate::crawler::HostConcurrency::adaptive),
/// each round of throttling doubles its gap, from the minimum, or one
/// second without one, up to the maximum. Each success halves it back to
/// the minimum. A `Retry-After` from the host pauses it for exactly as long
/// as it asks, beyond the maximum if need be, without widening its gap.
/// The next request waits for the gap after the last one started, as the
/// gap is when it is due, so a widened gap applies at once.
///
/// A [`Duration`] is the minimum, with the [default maximum](Self::DEFAULT_MAX). A
/// range sets both ends; the same value at both ends keeps the gap fixed.
///
/// ```
/// use std::time::Duration;
///
/// use spire_core::Crawler;
/// use spire_core::backend::Noop;
/// use spire_core::worker::from_fn;
///
/// let worker = from_fn(|_: spire_core::Context<Noop>| async {});
/// let crawler = Crawler::new(Noop, worker)
///     .with_host_delay(Duration::from_millis(200)..=Duration::from_secs(300));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostDelay {
    min: Duration,
    max: Duration,
}

impl HostDelay {
    /// The maximum gap when only a minimum is given: one request a minute.
    pub const DEFAULT_MAX: Duration = Duration::from_mins(1);

    /// A gap of at least `min`, growing up to `max`, or `min` if `max` is
    /// smaller.
    #[must_use]
    pub fn new(min: Duration, max: Duration) -> Self {
        Self {
            min,
            max: max.max(min),
        }
    }

    /// Returns the smallest gap.
    #[must_use]
    pub const fn min(self) -> Duration {
        self.min
    }

    /// Returns the largest gap.
    #[must_use]
    pub const fn max(self) -> Duration {
        self.max
    }

    /// Raises the smallest gap to at least `min`, e.g. for a robots.txt
    /// `Crawl-delay`.
    #[cfg(feature = "robots")]
    pub(crate) fn raise(&mut self, min: Duration) {
        *self = Self::new(self.min.max(min), self.max);
    }

    /// The gap after `gap` when the host is throttled: doubled, from the
    /// minimum, or one second if the minimum is zero, up to the maximum.
    pub(crate) fn grow(self, gap: Duration) -> Duration {
        (gap * 2).max(self.base()).min(self.max)
    }

    /// The gap after `gap` when the host recovers: halved, back to the
    /// minimum.
    pub(crate) fn shrink(self, gap: Duration) -> Duration {
        let half = gap / 2;
        if half < self.base() { self.min } else { half }
    }

    /// The smallest gap a throttled host gets.
    fn base(self) -> Duration {
        if self.min.is_zero() {
            Duration::from_secs(1)
        } else {
            self.min
        }
    }
}

impl Default for HostDelay {
    /// No gap, growing up to [`DEFAULT_MAX`](Self::DEFAULT_MAX) while a host is throttled.
    fn default() -> Self {
        Self::from(Duration::ZERO)
    }
}

impl From<Duration> for HostDelay {
    fn from(min: Duration) -> Self {
        Self::new(min, Self::DEFAULT_MAX)
    }
}

impl From<RangeInclusive<Duration>> for HostDelay {
    fn from(range: RangeInclusive<Duration>) -> Self {
        Self::new(*range.start(), *range.end())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn ms(ms: u64) -> Duration {
        Duration::from_millis(ms)
    }

    #[test]
    fn converts_from_a_minimum_or_a_range() {
        assert_eq!(
            HostDelay::from(ms(200)),
            HostDelay::new(ms(200), HostDelay::DEFAULT_MAX)
        );
        assert_eq!(HostDelay::from(ms(200)..=ms(300)).max(), ms(300));
        assert_eq!(HostDelay::from(ms(500)..=ms(300)).max(), ms(500), "fixed");
        assert_eq!(HostDelay::default(), HostDelay::from(Duration::ZERO));
    }

    #[test]
    fn grows_from_the_minimum_and_shrinks_back() {
        let delay = HostDelay::from(ms(200)..=ms(1000));
        let mut gap = delay.min();
        let mut gaps = Vec::new();
        for _ in 0..4 {
            gap = delay.grow(gap);
            gaps.push(gap);
        }
        assert_eq!(gaps, [ms(400), ms(800), ms(1000), ms(1000)]);
        gaps.clear();
        for _ in 0..4 {
            gap = delay.shrink(gap);
            gaps.push(gap);
        }
        assert_eq!(gaps, [ms(500), ms(250), ms(200), ms(200)]);
    }

    #[test]
    fn grows_from_a_second_without_a_minimum() {
        let delay = HostDelay::default();
        assert_eq!(delay.grow(Duration::ZERO), Duration::from_secs(1));
        assert_eq!(delay.grow(Duration::from_secs(1)), Duration::from_secs(2));
        assert_eq!(delay.grow(Duration::from_secs(50)), HostDelay::DEFAULT_MAX);
        assert_eq!(delay.shrink(Duration::from_secs(1)), Duration::ZERO);
        // A fixed gap never grows.
        let fixed = HostDelay::from(ms(200)..=ms(200));
        assert_eq!(fixed.grow(ms(200)), ms(200));
    }
}
