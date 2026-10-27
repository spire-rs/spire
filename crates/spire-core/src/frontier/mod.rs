//! The frontier: requests waiting to be crawled.
//!
//! A [`Frontier`] stores [`Seed`]s and decides which one is crawled next.
//! [`InMemFrontier`] provides the storage; facades wrap any frontier to add
//! behavior and are chained with [`FrontierExt`]:
//!
//! ```
//! use spire_core::frontier::{FrontierExt, InMemFrontier};
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
//! - [`Revisits`] crawls seeds again on a schedule, kept by storage that
//!   implements [`Schedule`].
//! - [`Scored`] adds a [`Score`] to every pushed seed's priority, for
//!   best-first crawling.
//!
//! Without a [`Retry`] facade, retries are refused and the seeds fail;
//! without a [`TagBackoff`] facade, backoffs only delay the retried request.

pub(crate) mod dynamic;
pub mod facade;
mod memory;
pub mod queue;
pub mod score;
pub mod seed;
pub mod seen;
#[cfg(test)]
pub(crate) mod test_util;

use std::future::{Future, ready};
use std::time::Duration;

use tokio::time::Instant;

use self::facade::{
    AllowedHosts, AvoidTraps, Dedup, Filter, MaxDepth, Retry, RetryPolicy, Revisits, Scored,
    TagBackoff, TrapPolicy,
};
pub use self::memory::InMemFrontier;
use self::score::Score;
use self::seed::Seed;
use self::seen::{InMemSeen, SeenSet};
use crate::Result;
use crate::crawler::{Revisit, Tag};

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
    fn pop(&self) -> impl Future<Output = Result<Pop>> + Send;

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

/// Storage that can hold seeds back until a time, for
/// [`FrontierExt::revisit`].
///
/// [`InMemFrontier`] keeps scheduled seeds in
/// memory, and a redb frontier in its file, so they survive a restart. It is
/// storage, not a facade: revisits sit directly on it, so every facade above
/// sees them as it sees queued seeds.
pub trait Schedule: Frontier {
    /// Holds `seed` back until `due`, then hands it out from
    /// [`pop`](Frontier::pop) like a queued seed. A seed with the same
    /// [fingerprint](seed::Seed::fingerprint) scheduled before is replaced,
    /// so a page has at most one revisit.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying storage fails.
    fn schedule(&self, seed: Seed, due: Instant) -> impl Future<Output = Result<()>> + Send;

    /// Returns when the next scheduled seed is due, if any.
    ///
    /// [`pop`](Frontier::pop) does not wait for scheduled seeds by itself, so
    /// a crawl over storage holding some ends once nothing else is queued;
    /// [`Revisits`] waits for them.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying storage fails.
    fn next_due(&self) -> impl Future<Output = Result<Option<Instant>>> + Send;
}

impl<F: Frontier> Frontier for std::sync::Arc<F> {
    fn push(&self, seed: Seed) -> impl Future<Output = Result<Admission>> + Send {
        F::push(self, seed)
    }

    fn pop(&self) -> impl Future<Output = Result<Pop>> + Send {
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

impl<F: Schedule> Schedule for std::sync::Arc<F> {
    fn schedule(&self, seed: Seed, due: Instant) -> impl Future<Output = Result<()>> + Send {
        F::schedule(self, seed, due)
    }

    fn next_due(&self) -> impl Future<Output = Result<Option<Instant>>> + Send {
        F::next_due(self)
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
    /// The seed was refused because its URL prefix looks like a scraper trap.
    SuspectedTrap,
    /// The seed was refused because its URL prefix exhausted a configured budget.
    PrefixBudgetExceeded,
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
pub enum Pop {
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

    /// Bounds expanding URL prefixes with conservative, configurable
    /// heuristics. Opt in alongside robots.txt and deduplication; legitimate
    /// large archives or opaque identifiers may need higher budgets.
    ///
    /// Put this outside retry/revisit facades to check queued seeds too.
    fn avoid_traps(self) -> AvoidTraps<Self> {
        self.avoid_traps_with(TrapPolicy::default())
    }

    /// Bounds expanding URL prefixes using `policy`, see [`Self::avoid_traps`].
    fn avoid_traps_with(self, policy: TrapPolicy) -> AvoidTraps<Self> {
        AvoidTraps::new(self, policy)
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

    /// Crawls pages again on a schedule: a seed reported done or failed is
    /// crawled again after its tag's [`Revisit::interval`], so the crawl
    /// keeps going, waiting for the next revisit once nothing else is
    /// queued.
    ///
    /// It goes directly on the storage, which keeps the schedule, e.g.
    /// `InMemFrontier::fifo().revisit::<Page>().dedup()`.
    fn revisit<T: Revisit>(self) -> Revisits<Self, T>
    where
        Self: Schedule,
    {
        Revisits::new(self)
    }

    /// Adds `scorer`'s score to the priority of every seed pushed, so a
    /// frontier that orders by priority crawls the best seeds first.
    ///
    /// The score adds to the seed's own [`priority`](Seed::with_priority),
    /// e.g. one a handler gave a link, and stacked scorers add up. Seeds
    /// are scored once, when pushed: retries and revisits keep their
    /// priority. A FIFO or LIFO frontier ignores priorities.
    ///
    /// ```
    /// use spire_core::frontier::score::{Keywords, ShallowFirst};
    /// use spire_core::frontier::{FrontierExt, InMemFrontier};
    ///
    /// let frontier = InMemFrontier::priority()
    ///     .dedup()
    ///     .score(Keywords::new(["laptop", "review"]).with_weight(10))
    ///     .score(ShallowFirst);
    /// ```
    fn score<S: Score>(self, scorer: S) -> Scored<Self, S> {
        Scored::new(self, scorer)
    }

    /// Wraps the frontier with a custom facade.
    fn layer<L: FrontierLayer<Self>>(self, layer: L) -> L::Frontier {
        layer.layer(self)
    }
}

impl<F: Frontier> FrontierExt for F {}
