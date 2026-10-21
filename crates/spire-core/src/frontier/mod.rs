//! The frontier: requests waiting to be crawled.
//!
//! A [`Frontier`] stores [`Seed`]s and decides which one is crawled next.
//! [`InMemFrontier`](memory::InMemFrontier) provides the storage; facades wrap any frontier to add
//! behavior and are chained with [`FrontierExt`]:
//!
//! ```
//! use spire_core::frontier::FrontierExt;
//! use spire_core::frontier::memory::InMemFrontier;
//!
//! let frontier = InMemFrontier::fifo()
//!     .dedup()
//!     .max_depth(5)
//!     .retry(3)
//!     .tag_backoff();
//! ```
//!
//! Pushes flow from the outermost facade inwards, so above, a seed that is
//! too deep is refused before it is marked as seen.
//!
//! - [`Filter`] refuses seeds outside the crawl's scope, e.g. other hosts.
//! - [`Dedup`] refuses seeds whose [`Fingerprint`](seed::Fingerprint) was seen before.
//! - [`MaxDepth`] refuses seeds deeper than a limit.
//! - [`Retry`] re-queues seeds after [`Report::Retry`], with a delay.
//! - [`TagBackoff`] holds back a tag's seeds after [`Report::Backoff`].
//!
//! Without a [`Retry`] facade, retries are refused and the seeds fail;
//! without a [`TagBackoff`] facade, backoffs only delay the retried request.

pub(crate) mod dynamic;
pub mod facade;
pub mod memory;
pub mod queue;
pub mod seed;
pub mod seen;
#[cfg(test)]
pub(crate) mod test_util;

use std::future::{Future, ready};
use std::time::Duration;

use tokio::time::Instant;

use self::facade::{AllowedHosts, Dedup, Filter, MaxDepth, Retry, RetryPolicy, TagBackoff};
use self::seed::Seed;
use self::seen::{InMemSeen, SeenSet};
use crate::Result;
use crate::request::Tag;

/// Stores [`Seed`]s and decides which one is crawled next.
pub trait Frontier: Send + Sync + 'static {
    /// Offers a seed, which the frontier may refuse.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying storage fails.
    fn push(&self, seed: Seed) -> impl Future<Output = Result<Admission>> + Send;

    /// Takes the next seed that is ready to be crawled.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying storage fails.
    fn pop(&self) -> impl Future<Output = Result<Next>> + Send;

    /// Tells the frontier how a popped seed ended.
    ///
    /// Facades react to the reports they handle and pass the rest inwards.
    /// The default implementation ignores the report, refusing retries.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying storage fails.
    fn report(&self, report: Report) -> impl Future<Output = Result<Ack>> + Send {
        let ack = match report {
            Report::Retry { .. } => Ack::RetryRefused,
            _ => Ack::Applied,
        };
        ready(Ok(ack))
    }

    /// Returns the number of queued seeds, if known.
    fn size_hint(&self) -> Option<usize> {
        None
    }

    /// Returns a copy of every seed held, in no particular order, to save
    /// the crawl and resume it later; `None` if the frontier cannot, e.g.
    /// because it persists seeds itself.
    ///
    /// Facades add the seeds they hold, such as seeds waiting for a retry.
    fn snapshot(&self) -> Option<Vec<Seed>> {
        None
    }
}

impl<F: Frontier> Frontier for std::sync::Arc<F> {
    fn push(&self, seed: Seed) -> impl Future<Output = Result<Admission>> + Send {
        F::push(self, seed)
    }

    fn pop(&self) -> impl Future<Output = Result<Next>> + Send {
        F::pop(self)
    }

    fn report(&self, report: Report) -> impl Future<Output = Result<Ack>> + Send {
        F::report(self, report)
    }

    fn size_hint(&self) -> Option<usize> {
        F::size_hint(self)
    }

    fn snapshot(&self) -> Option<Vec<Seed>> {
        F::snapshot(self)
    }
}

/// Whether [`Frontier::push`] queued a seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Admission {
    /// The seed was queued.
    Queued,
    /// The seed was refused because an identical one was seen before.
    Duplicate,
    /// The seed was refused because it is deeper than allowed.
    TooDeep,
    /// The link was skipped because it is not an `http(s)` URL.
    Unsupported,
    /// The seed was refused because it is outside the crawl's scope.
    OutOfScope,
}

impl Admission {
    /// Whether the seed was queued.
    #[must_use]
    pub const fn is_queued(self) -> bool {
        matches!(self, Self::Queued)
    }
}

/// The result of [`Frontier::pop`].
#[derive(Debug)]
pub enum Next {
    /// A seed is ready to be crawled.
    Ready(Seed),
    /// No seed is ready now, but one will be at the given instant.
    Wait(Instant),
    /// No seed is queued.
    Empty,
}

/// How a popped seed ended, see [`Frontier::report`].
#[derive(Debug)]
#[non_exhaustive]
pub enum Report {
    /// The seed was crawled.
    Done(Seed),
    /// The seed failed and will not be tried again.
    Failed(Seed),
    /// The seed was not crawled on purpose, e.g. robots.txt disallows it or
    /// its tag was aborted.
    Skipped(Seed),
    /// The crawl stopped before the seed finished; a persistent frontier
    /// should keep it for the next run.
    Cancelled(Seed),
    /// The seed should be tried again.
    Retry {
        /// The seed that failed.
        seed: Seed,
        /// Delay before the next attempt; `None` defers to the frontier.
        after: Option<Duration>,
    },
    /// Seeds with `tag` should not be crawled before `until`.
    Backoff {
        /// The tag to pause; `None` pauses untagged seeds.
        tag: Option<Tag>,
        /// The earliest instant at which the tag may be crawled again.
        until: Instant,
    },
}

/// What a frontier did with a [`Report`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Ack {
    /// The report was applied.
    Applied,
    /// A [`Report::Retry`] was refused, because the seed is out of retries or
    /// the frontier cannot retry; the seed has failed.
    RetryRefused,
}

/// Wraps a [`Frontier`] in another, see [`FrontierExt::layer`].
pub trait FrontierLayer<F> {
    /// The wrapping frontier.
    type Frontier: Frontier;

    /// Wraps `inner`.
    fn layer(self, inner: F) -> Self::Frontier;
}

/// Chains facades onto a [`Frontier`].
pub trait FrontierExt: Frontier + Sized {
    /// Refuses seeds seen before, remembering them in memory.
    fn dedup(self) -> Dedup<Self> {
        Dedup::new(self, InMemSeen::default())
    }

    /// Refuses seeds seen before, remembering them in `seen`.
    fn dedup_with<S: SeenSet>(self, seen: S) -> Dedup<Self, S> {
        Dedup::new(self, seen)
    }

    /// Refuses seeds `predicate` rejects.
    fn filter<P>(self, predicate: P) -> Filter<Self, P>
    where
        P: Fn(&Seed) -> bool + Send + Sync + 'static,
    {
        Filter::new(self, predicate)
    }

    /// Refuses seeds outside `hosts` and their subdomains, e.g. to keep a
    /// crawl from following links off-site.
    fn allow_hosts<I>(
        self,
        hosts: I,
    ) -> Filter<Self, impl Fn(&Seed) -> bool + Send + Sync + 'static>
    where
        I: IntoIterator<Item: AsRef<str>>,
    {
        let hosts = AllowedHosts::new(hosts);
        Filter::new(self, move |seed: &Seed| hosts.allows(seed))
    }

    /// Refuses seeds deeper than `depth`.
    fn max_depth(self, depth: u32) -> MaxDepth<Self> {
        MaxDepth::new(self, depth)
    }

    /// Retries failed seeds up to `max_retries` times with exponential
    /// backoff.
    fn retry(self, max_retries: u32) -> Retry<Self> {
        Retry::new(self, RetryPolicy::new(max_retries))
    }

    /// Retries failed seeds according to `policy`.
    fn retry_with(self, policy: RetryPolicy) -> Retry<Self> {
        Retry::new(self, policy)
    }

    /// Holds back seeds of paused tags.
    fn tag_backoff(self) -> TagBackoff<Self> {
        TagBackoff::new(self)
    }

    /// Wraps the frontier with a custom facade.
    fn layer<L: FrontierLayer<Self>>(self, layer: L) -> L::Frontier {
        layer.layer(self)
    }
}

impl<F: Frontier> FrontierExt for F {}
