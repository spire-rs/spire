//! robots.txt, fetched once per origin and obeyed per RFC 9309.

mod cache;
mod source;

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

pub(crate) use cache::{RobotsCache, Verdict};
pub(crate) use source::{BackendSource, RobotsSource};

use crate::backend::Backend;
use crate::crawler::connector::Connector;
use crate::request::Tag;

/// How a crawl fetches and obeys robots.txt, see
/// [`Crawler::with_robots`](crate::Crawler::with_robots).
///
/// robots.txt is fetched once per origin (scheme, host and port) before its
/// first request, and handled as RFC 9309 describes:
///
/// - Its rules for [`user agent`](Self::new) decide what is crawled;
///   disallowed requests are skipped, and `Crawl-delay` slows the host down.
/// - A `4xx` other than `429`, or too many redirects, allows everything.
/// - A `5xx`, a `429` or a network failure disallows everything while
///   robots.txt is retried with backoff. Once the
///   [retries](Self::with_unreachable_retries) are spent, the origin's
///   requests are skipped.
/// - Rules older than the [maximum age](Self::with_max_age) are fetched
///   again; until then, and if fetching fails, the old rules apply.
///
/// robots.txt is fetched with the crawl's backend unless
/// [`with_backend`](Self::with_backend) sets another, e.g. an HTTP client
/// for a browser crawl. [`with_sitemaps`](Self::with_sitemaps) also queues
/// the sitemaps it lists.
///
/// ```
/// use std::time::Duration;
///
/// use spire_core::backend::Noop;
/// use spire_core::crawler::Robots;
///
/// let robots = Robots::new("MyBot")
///     .with_backend(Noop)
///     .with_max_age(Duration::from_secs(3600));
/// ```
#[derive(Clone)]
pub struct Robots {
    pub(crate) agent: Arc<str>,
    pub(crate) source: Option<Arc<dyn RobotsSource>>,
    pub(crate) max_age: Duration,
    pub(crate) retries: u32,
    pub(crate) retry_delay: Duration,
    pub(crate) sitemaps: Option<Tag>,
}

impl Robots {
    /// Obeys robots.txt for `user_agent`, fetching it with the crawl's
    /// backend, keeping it for 24 hours and retrying an unreachable one 3
    /// times.
    pub fn new(user_agent: impl Into<Arc<str>>) -> Self {
        Self {
            agent: user_agent.into(),
            source: None,
            max_age: Duration::from_hours(24),
            retries: 3,
            retry_delay: Duration::from_secs(30),
            sitemaps: None,
        }
    }

    /// Fetches robots.txt with `backend` instead of the crawl's.
    #[must_use]
    pub fn with_backend<B: Backend>(mut self, backend: B) -> Self {
        let connector = Connector::new(Arc::new(backend));
        self.source = Some(Arc::new(BackendSource::new(Arc::new(connector))));
        self
    }

    /// Fetches robots.txt again once its rules are older than `age`.
    /// RFC 9309 asks for at most 24 hours.
    #[must_use]
    pub const fn with_max_age(mut self, age: Duration) -> Self {
        self.max_age = age;
        self
    }

    /// Retries an unreachable robots.txt `retries` times, 30 seconds apart
    /// and doubling, before skipping the origin's requests.
    #[must_use]
    pub const fn with_unreachable_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }

    /// Queues the sitemaps an origin's robots.txt lists in `Sitemap:` lines,
    /// tagged `tag`, the first time it is fetched.
    ///
    /// They go through the frontier like any other seed, so its scope,
    /// deduplication and depth limits apply. Route the tag to a handler that
    /// parses them, e.g. `FollowSitemap` in `spire`.
    #[must_use]
    pub fn with_sitemaps(mut self, tag: impl Into<Tag>) -> Self {
        self.sitemaps = Some(tag.into());
        self
    }

    /// The delay before the `attempt`-th retry, starting at 1.
    pub(crate) fn retry_delay(&self, attempt: u32) -> Duration {
        let exponent = attempt.saturating_sub(1).min(16);
        self.retry_delay
            .saturating_mul(1 << exponent)
            .min(Duration::from_mins(10))
    }
}

impl From<&str> for Robots {
    fn from(user_agent: &str) -> Self {
        Self::new(user_agent)
    }
}

impl From<String> for Robots {
    fn from(user_agent: String) -> Self {
        Self::new(user_agent)
    }
}

impl fmt::Debug for Robots {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Robots")
            .field("agent", &self.agent)
            .field("own_backend", &self.source.is_some())
            .field("max_age", &self.max_age)
            .field("retries", &self.retries)
            .field("retry_delay", &self.retry_delay)
            .field("sitemaps", &self.sitemaps)
            .finish()
    }
}
