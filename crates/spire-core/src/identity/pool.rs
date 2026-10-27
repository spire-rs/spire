//! The identities of a run: picking one per request, and taking failing
//! ones out of use.

use std::collections::HashMap;
use std::hash::{BuildHasher, RandomState};
use std::sync::Mutex;

use ::http::StatusCode;
use tokio::time::Instant;

use super::Identity;
use super::identities::{Choice, Identities, Source};
use crate::frontier::seed::Seed;
use crate::sync::lock;
use crate::worker::Outcome;
use crate::{Error, ErrorKind, Result, TRACING_TARGET};

/// Set-aside periods double at most this many times.
const MAX_DOUBLINGS: u32 = 10;

/// How a request went, as far as its identity is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Health {
    /// The identity got an answer it was not refused with.
    Ok,
    /// The identity could not connect, timed out, or was refused: `403`,
    /// `407` or `429`.
    Failed,
    /// Nothing is known about the identity, e.g. a handler failed without
    /// fetching.
    Unknown,
}

impl Health {
    /// Classifies a finished request from its outcome and, if it was
    /// fetched, its response status.
    pub(crate) fn of(outcome: &Outcome, status: Option<StatusCode>) -> Self {
        let error = match outcome {
            Outcome::Fail(error) => Some(error),
            _ => None,
        };
        let status = status.or_else(|| error.and_then(crate::Error::status));
        match status {
            Some(
                StatusCode::FORBIDDEN
                | StatusCode::PROXY_AUTHENTICATION_REQUIRED
                | StatusCode::TOO_MANY_REQUESTS,
            ) => Self::Failed,
            Some(_) => Self::Ok,
            // No answer at all: the connection, the proxy or the transfer
            // failed.
            None if error.is_some_and(|error| {
                matches!(
                    error.kind(),
                    ErrorKind::Http | ErrorKind::Timeout | ErrorKind::Backend
                )
            }) =>
            {
                Self::Failed
            }
            None => Self::Unknown,
        }
    }
}

/// The identities of a run, see [`Identities`].
pub(crate) struct IdentityPool {
    config: Identities,
    state: Mutex<State>,
}

struct State {
    /// The identities in use, or set aside.
    slots: Vec<Slot>,
    /// The next identity to generate.
    next: u64,
    /// The next turn, for picking in turn.
    turn: usize,
    /// The identity of each host, for sticky picking.
    hosts: HashMap<String, Box<str>>,
    /// Seeds random picks.
    random: RandomState,
}

struct Slot {
    identity: Identity,
    created: Instant,
    requests: u64,
    /// Failures in a row.
    failures: u32,
    /// Until when it is set aside.
    resting_until: Option<Instant>,
    /// How many times it was set aside in a row.
    rests: u32,
}

impl Slot {
    fn new(identity: Identity, now: Instant) -> Self {
        Self {
            identity,
            created: now,
            requests: 0,
            failures: 0,
            resting_until: None,
            rests: 0,
        }
    }

    fn available(&self, now: Instant) -> bool {
        self.resting_until.is_none_or(|until| until <= now)
    }
}

impl IdentityPool {
    pub(crate) fn new(config: Identities) -> Self {
        let now = Instant::now();
        let slots = match &config.source {
            Source::Fixed(identities) => identities
                .iter()
                .map(|identity| Slot::new(identity.clone(), now))
                .collect(),
            Source::Generate(_) => Vec::new(),
        };
        let state = State {
            slots,
            next: 0,
            turn: 0,
            hosts: HashMap::new(),
            random: RandomState::new(),
        };
        Self {
            config,
            state: Mutex::new(state),
        }
    }

    /// Picks the identity `seed` goes out as.
    ///
    /// # Errors
    ///
    /// Returns an [`Invalid`](ErrorKind::Invalid) error if the seed is
    /// pinned to an identity not in use, a retryable
    /// [`Backend`](ErrorKind::Backend) error if every identity is set
    /// aside, or the error of building a new one.
    pub(crate) fn choose(&self, seed: &Seed, now: Instant) -> Result<Identity> {
        let mut state = lock(&self.state);

        if let Some(name) = seed.identity() {
            let slot = state
                .slots
                .iter_mut()
                .find(|slot| slot.identity.name() == name)
                .ok_or_else(|| Error::invalid(format!("no identity named `{name}` is in use")))?;
            slot.requests += 1;
            return Ok(slot.identity.clone());
        }

        self.retire_old(&mut state, now);
        let host = seed.uri().host().map(str::to_ascii_lowercase);
        let sticky = self.config.choice == Choice::StickyPerHost;

        let kept = host
            .as_ref()
            .filter(|_| sticky)
            .and_then(|host| state.hosts.get(host))
            .and_then(|name| {
                state
                    .slots
                    .iter()
                    .position(|slot| *slot.identity.name() == **name && slot.available(now))
            });
        let index = match kept {
            Some(index) => index,
            None => self.pick(&mut state, now)?,
        };

        if sticky && let Some(host) = host {
            let name = state.slots[index].identity.name().into();
            state.hosts.insert(host, name);
        }
        let slot = &mut state.slots[index];
        slot.requests += 1;
        Ok(slot.identity.clone())
    }

    /// Picks an identity for a request no identity is kept for: a new one
    /// while fewer than `max_active` are in use, or else one in use.
    fn pick(&self, state: &mut State, now: Instant) -> Result<usize> {
        if let Source::Generate(generate) = &self.config.source
            && state.slots.len() < self.config.max_active
        {
            let n = state.next;
            state.next += 1;
            let identity = generate(n)?;
            tracing::debug!(target: TRACING_TARGET, identity = identity.name(), "identity created");
            state.slots.push(Slot::new(identity, now));
            return Ok(state.slots.len() - 1);
        }

        let available: Vec<usize> = (0..state.slots.len())
            .filter(|&index| state.slots[index].available(now))
            .collect();
        if available.is_empty() {
            let back = state
                .slots
                .iter()
                .filter_map(|slot| slot.resting_until)
                .min()
                .map_or_else(Default::default, |until| {
                    until.saturating_duration_since(now)
                });
            return Err(Error::new(ErrorKind::Backend)
                .with_message("every identity is set aside")
                .with_retry_after(back));
        }

        let turn = match self.config.choice {
            Choice::Random => {
                let draw = state.random.hash_one(state.turn);
                usize::try_from(draw % available.len() as u64).unwrap_or_default()
            }
            Choice::InTurn | Choice::StickyPerHost => state.turn % available.len(),
        };
        state.turn = state.turn.wrapping_add(1);
        Ok(available[turn])
    }

    /// Retires generated identities past their request or age limit.
    fn retire_old(&self, state: &mut State, now: Instant) {
        if !matches!(self.config.source, Source::Generate(_)) {
            return;
        }
        let Identities {
            max_requests,
            max_age,
            ..
        } = self.config;
        state.slots.retain(|slot| {
            let spent = max_requests.is_some_and(|max| slot.requests >= max);
            let old = max_age.is_some_and(|max| now.saturating_duration_since(slot.created) >= max);
            if spent || old {
                tracing::debug!(target: TRACING_TARGET, identity = slot.identity.name(), "identity retired");
            }
            !(spent || old)
        });
    }

    /// Records how a request that went out as `identity` went: a failing
    /// identity is retired if generated, or else set aside.
    pub(crate) fn finished(&self, identity: &Identity, health: Health, now: Instant) {
        let mut state = lock(&self.state);
        let Some(index) = state
            .slots
            .iter()
            .position(|slot| slot.identity == *identity)
        else {
            // Retired meanwhile.
            return;
        };
        let slot = &mut state.slots[index];
        match health {
            Health::Unknown => {}
            Health::Ok => {
                slot.failures = 0;
                slot.rests = 0;
            }
            Health::Failed => {
                slot.failures += 1;
                if slot.failures < self.config.max_failures {
                    return;
                }
                if matches!(self.config.source, Source::Generate(_)) {
                    tracing::info!(target: TRACING_TARGET, identity = identity.name(), "identity failing, retiring it");
                    state.slots.remove(index);
                    return;
                }
                let rest = self.config.rest * 2u32.pow(slot.rests.min(MAX_DOUBLINGS));
                tracing::info!(
                    target: TRACING_TARGET,
                    identity = identity.name(),
                    rest_secs = rest.as_secs(),
                    "identity failing, setting it aside",
                );
                slot.failures = 0;
                slot.rests += 1;
                slot.resting_until = Some(now + rest);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn seed(uri: &str) -> Seed {
        Seed::get(uri).unwrap()
    }

    fn generated() -> Identities {
        Identities::from_fn(|n| Ok(Identity::new(format!("id-{n}"))))
    }

    fn names(pool: &IdentityPool, uris: &[&str], now: Instant) -> Vec<String> {
        uris.iter()
            .map(|uri| pool.choose(&seed(uri), now).unwrap().name().to_owned())
            .collect()
    }

    #[test]
    fn generates_up_to_max_active_then_takes_turns() {
        let pool = IdentityPool::new(generated().max_active(2));
        let now = Instant::now();
        let uris = ["https://a.test/"; 5];
        assert_eq!(
            names(&pool, &uris, now),
            ["id-0", "id-1", "id-0", "id-1", "id-0"]
        );
    }

    #[test]
    fn sticky_keeps_each_host_on_one_identity() {
        let pool = IdentityPool::new(generated().max_active(2).sticky_per_host());
        let now = Instant::now();
        let uris = [
            "https://a.test/1",
            "https://b.test/1",
            "https://A.test/2",
            "https://c.test/",
            "https://b.test/2",
        ];
        let picked = names(&pool, &uris, now);
        assert_eq!(picked[..3], ["id-0", "id-1", "id-0"]);
        assert_eq!(picked[4], "id-1");
    }

    #[test]
    fn failing_generated_identities_are_replaced() {
        let pool = IdentityPool::new(generated().max_active(1).max_failures(2));
        let now = Instant::now();
        let first = pool.choose(&seed("https://a.test/"), now).unwrap();
        pool.finished(&first, Health::Failed, now);
        pool.finished(&first, Health::Ok, now);
        pool.finished(&first, Health::Failed, now);
        assert_eq!(names(&pool, &["https://a.test/"], now), ["id-0"]);

        pool.finished(&first, Health::Failed, now);
        assert_eq!(names(&pool, &["https://a.test/"], now), ["id-1"]);
    }

    #[test]
    fn failing_listed_identities_rest_longer_each_time() {
        let identities = Identities::new([Identity::new("a"), Identity::new("b")])
            .max_failures(1)
            .rest(Duration::from_secs(10));
        let pool = IdentityPool::new(identities);
        let now = Instant::now();
        let a = Identity::new("a");

        pool.finished(&a, Health::Failed, now);
        assert_eq!(names(&pool, &["https://x.test/"; 2], now), ["b", "b"]);
        let later = now + Duration::from_secs(10);
        assert!(names(&pool, &["https://x.test/"; 2], later).contains(&"a".to_owned()));

        // Set aside again, for twice as long.
        pool.finished(&a, Health::Failed, later);
        let soon = later + Duration::from_secs(10);
        assert_eq!(names(&pool, &["https://x.test/"; 2], soon), ["b", "b"]);
    }

    #[test]
    fn every_identity_resting_is_a_retryable_error() {
        let identities = Identities::new([Identity::new("a")])
            .max_failures(1)
            .rest(Duration::from_secs(30));
        let pool = IdentityPool::new(identities);
        let now = Instant::now();
        pool.finished(&Identity::new("a"), Health::Failed, now);

        let error = pool.choose(&seed("https://x.test/"), now).unwrap_err();
        assert!(error.is_retryable());
        assert_eq!(error.retry_after(), Some(Duration::from_secs(30)));
    }

    #[test]
    fn identities_retire_after_their_requests() {
        let pool = IdentityPool::new(generated().max_active(1).retire_after_requests(2));
        let now = Instant::now();
        let uris = ["https://a.test/"; 5];
        assert_eq!(
            names(&pool, &uris, now),
            ["id-0", "id-0", "id-1", "id-1", "id-2"]
        );
    }

    #[test]
    fn pinned_seeds_use_their_identity() {
        let pool = IdentityPool::new(Identities::new([Identity::new("a"), Identity::new("b")]));
        let now = Instant::now();
        let pinned = seed("https://x.test/").with_identity("b");
        assert_eq!(pool.choose(&pinned, now).unwrap().name(), "b");

        let unknown = seed("https://x.test/").with_identity("c");
        assert_eq!(
            pool.choose(&unknown, now).unwrap_err().kind(),
            ErrorKind::Invalid
        );
    }

    #[test]
    fn classifies_health() {
        let fail = |kind| Outcome::Fail(Error::new(kind));
        assert_eq!(
            Health::of(&Outcome::Continue, Some(StatusCode::OK)),
            Health::Ok
        );
        assert_eq!(
            Health::of(&Outcome::Continue, Some(StatusCode::FORBIDDEN)),
            Health::Failed
        );
        assert_eq!(Health::of(&fail(ErrorKind::Timeout), None), Health::Failed);
        assert_eq!(Health::of(&fail(ErrorKind::Worker), None), Health::Unknown);
        assert_eq!(
            Health::of(&fail(ErrorKind::Rejected), Some(StatusCode::NOT_FOUND)),
            Health::Ok
        );
    }
}
