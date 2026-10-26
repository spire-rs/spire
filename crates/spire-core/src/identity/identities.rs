//! [`Identities`]: where a crawl's identities come from, and how they are
//! picked and taken out of use.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use super::Identity;
use crate::Result;

/// Builds identity `n`, for [`Identities::from_fn`].
pub(super) type Generate = Arc<dyn Fn(u64) -> Result<Identity> + Send + Sync>;

/// Where a crawl's identities come from.
#[derive(Clone)]
pub(super) enum Source {
    /// A fixed list.
    Fixed(Vec<Identity>),
    /// Built on demand, identity `n` by a function.
    Generate(Generate),
}

/// How an identity is picked for a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Choice {
    InTurn,
    Random,
    StickyPerHost,
}

/// The identities of a crawl, and how they are picked, set aside and replaced.
///
/// A request pinned with
/// [`Seed::with_identity`](crate::frontier::seed::Seed::with_identity) goes
/// out as that identity; any other gets one picked [in turn](Self::in_turn),
/// [at random](Self::at_random), or [per host](Self::sticky_per_host).
///
/// An identity whose requests fail [`max_failures`](Self::max_failures)
/// times in a row, through connection failures, timeouts, or `403`, `407`
/// or `429` answers, is taken out of use: a generated one is retired and
/// replaced by the next, a listed one set aside for a while, longer each
/// time.
#[derive(Clone)]
pub struct Identities {
    pub(super) source: Source,
    pub(super) max_active: usize,
    pub(super) choice: Choice,
    pub(super) max_requests: Option<u64>,
    pub(super) max_age: Option<Duration>,
    pub(super) max_failures: u32,
    pub(super) rest: Duration,
}

impl Identities {
    /// The identities generated at once by default, see
    /// [`max_active`](Self::max_active).
    pub const DEFAULT_MAX_ACTIVE: usize = 16;

    /// Uses a fixed list of identities.
    ///
    /// # Panics
    ///
    /// Panics if `identities` is empty or two share a name.
    #[must_use]
    pub fn new(identities: impl IntoIterator<Item = Identity>) -> Self {
        let identities: Vec<Identity> = identities.into_iter().collect();
        assert!(
            !identities.is_empty(),
            "`Identities::new` needs an identity"
        );
        for (index, identity) in identities.iter().enumerate() {
            assert!(
                !identities[..index].contains(identity),
                "two identities are named `{}`",
                identity.name()
            );
        }
        let max_active = identities.len();
        Self::with_source(Source::Fixed(identities), max_active)
    }

    /// Builds identities on demand: identity `n`, counting from 0, from
    /// `generate`, e.g. a gateway's session `n`, or the `n`th proxy of a
    /// list. Up to [`max_active`](Self::max_active) are in use at once;
    /// one taken out of use is replaced by the next `n`.
    ///
    /// Names should follow from `n`, so a resumed crawl builds the same
    /// identities again. A failure to build one fails the request that
    /// needed it.
    #[must_use]
    pub fn from_fn<F>(generate: F) -> Self
    where
        F: Fn(u64) -> Result<Identity> + Send + Sync + 'static,
    {
        Self::with_source(
            Source::Generate(Arc::new(generate)),
            Self::DEFAULT_MAX_ACTIVE,
        )
    }

    const fn with_source(source: Source, max_active: usize) -> Self {
        Self {
            source,
            max_active,
            choice: Choice::InTurn,
            max_requests: None,
            max_age: None,
            max_failures: 3,
            rest: Duration::from_mins(1),
        }
    }

    /// Generates at most `n` identities at once, created as requests need
    /// them, so a thousand cost nothing until used. Ignored for a fixed
    /// list.
    ///
    /// # Panics
    ///
    /// Panics if `n` is zero.
    #[must_use]
    pub fn max_active(mut self, n: usize) -> Self {
        assert!(n > 0, "`Identities::max_active` must be above zero");
        if matches!(self.source, Source::Generate(_)) {
            self.max_active = n;
        }
        self
    }

    /// Picks identities in turn, one request each. This is the default.
    #[must_use]
    pub const fn in_turn(mut self) -> Self {
        self.choice = Choice::InTurn;
        self
    }

    /// Picks an identity at random for every request.
    #[must_use]
    pub const fn at_random(mut self) -> Self {
        self.choice = Choice::Random;
        self
    }

    /// Keeps each host on one identity, as one visitor would be, for as
    /// long as that identity stays in use; a new host gets a new identity
    /// while fewer than [`max_active`](Self::max_active) are.
    #[must_use]
    pub const fn sticky_per_host(mut self) -> Self {
        self.choice = Choice::StickyPerHost;
        self
    }

    /// Retires a generated identity after `n` requests, replacing it with
    /// the next.
    #[must_use]
    pub const fn retire_after_requests(mut self, n: u64) -> Self {
        self.max_requests = Some(n);
        self
    }

    /// Retires a generated identity once it is `age` old, replacing it with
    /// the next.
    #[must_use]
    pub const fn retire_after(mut self, age: Duration) -> Self {
        self.max_age = Some(age);
        self
    }

    /// Takes an identity out of use after `n` failures in a row, 3 by
    /// default.
    ///
    /// # Panics
    ///
    /// Panics if `n` is zero.
    #[must_use]
    pub fn max_failures(mut self, n: u32) -> Self {
        assert!(n > 0, "`Identities::max_failures` must be above zero");
        self.max_failures = n;
        self
    }

    /// Sets a failing listed identity aside for `duration` the first time,
    /// one minute by default, and twice as long each time after.
    #[must_use]
    pub const fn rest(mut self, duration: Duration) -> Self {
        self.rest = duration;
        self
    }
}

impl fmt::Debug for Identities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let source = match &self.source {
            Source::Fixed(identities) => format!("{} listed", identities.len()),
            Source::Generate(_) => "generated".to_owned(),
        };
        f.debug_struct("Identities")
            .field("source", &source)
            .field("max_active", &self.max_active)
            .field("choice", &self.choice)
            .field("max_requests", &self.max_requests)
            .field("max_age", &self.max_age)
            .field("max_failures", &self.max_failures)
            .field("rest", &self.rest)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "two identities are named `a`")]
    fn listed_names_are_unique() {
        let _ = Identities::new([Identity::new("a"), Identity::new("a")]);
    }
}
