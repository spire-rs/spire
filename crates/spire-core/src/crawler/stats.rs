use std::time::Duration;

/// Totals of a [`Crawler::run`](crate::crawler::Crawler::run), live while it runs
/// through [`Crawler::stats`](crate::crawler::Crawler::stats).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Stats {
    /// Requests handled, counting every attempt.
    pub requests: usize,
    /// Requests that finished with [`Outcome::Continue`] or
    /// [`Outcome::Stop`].
    ///
    /// [`Outcome::Continue`]: crate::worker::Outcome::Continue
    /// [`Outcome::Stop`]: crate::worker::Outcome::Stop
    pub succeeded: usize,
    /// Requests that failed for good: not retryable, out of retries, or
    /// refused a retry by the frontier.
    pub failed: usize,
    /// Requests the frontier accepted for another attempt.
    pub retried: usize,
    /// Requests not crawled on purpose: disallowed by robots.txt or with an
    /// aborted tag.
    pub skipped: usize,
    /// Requests still in flight when the shutdown grace period ended.
    pub cancelled: usize,
    /// Requests the server answered `304 Not Modified`: unchanged since the
    /// crawl last saw them, see `Revalidate`.
    pub unchanged: usize,
    /// Wall-clock time of the run.
    pub elapsed: Duration,
}
