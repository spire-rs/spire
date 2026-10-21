//! Per-host politeness: concurrency and delays.

use std::collections::{HashMap, VecDeque};
use std::num::NonZeroUsize;
use std::time::Duration;

use tokio::time::Instant;

use crate::frontier::Seed;

/// How a crawl treats every host.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct HostPolicy {
    /// Requests in flight per host at most; unlimited if `None`.
    pub(crate) max_concurrency: Option<NonZeroUsize>,
    /// Time between the starts of two requests to the same host.
    pub(crate) delay: Duration,
}

/// What to do with a seed that is allowed to be crawled.
#[expect(clippy::large_enum_variant, reason = "moved once per admitted seed")]
pub(crate) enum Admit {
    /// Start it now.
    Start(Seed),
    /// Its host is busy; it waits in the host's queue.
    Parked,
}

#[derive(Default)]
struct Host {
    in_flight: usize,
    next_start: Option<Instant>,
    delay: Duration,
    parked: VecDeque<Seed>,
}

/// The state of every host a run has seen.
///
/// Hosts are keyed by authority, so `http://` and `https://` of the same
/// host share limits: they reach the same server.
pub(crate) struct Hosts {
    policy: HostPolicy,
    by_key: HashMap<String, Host>,
    parked: usize,
}

/// The key a seed's host is tracked under: its lowercase authority.
pub(crate) fn key(seed: &Seed) -> String {
    seed.uri()
        .authority()
        .map(|authority| authority.as_str().to_ascii_lowercase())
        .unwrap_or_default()
}

impl Hosts {
    pub(crate) fn new(policy: HostPolicy) -> Self {
        Self {
            policy,
            by_key: HashMap::new(),
            parked: 0,
        }
    }

    fn host(&mut self, key: &str) -> &mut Host {
        let delay = self.policy.delay;
        self.by_key.entry(key.to_owned()).or_insert_with(|| Host {
            delay,
            ..Host::default()
        })
    }

    fn is_free(policy: HostPolicy, host: &Host, now: Instant) -> bool {
        let capacity = policy
            .max_concurrency
            .is_none_or(|max| host.in_flight < max.get());
        let due = host.next_start.is_none_or(|at| at <= now);
        capacity && due
    }

    /// Decides whether `seed` starts now or waits for its host.
    pub(crate) fn admit(&mut self, seed: Seed, now: Instant) -> Admit {
        let policy = self.policy;
        let host = self.host(&key(&seed));
        if host.parked.is_empty() && Self::is_free(policy, host, now) {
            return Admit::Start(seed);
        }

        host.parked.push_back(seed);
        self.parked += 1;
        Admit::Parked
    }

    /// Takes a parked seed whose host is free.
    pub(crate) fn take_ready(&mut self, now: Instant) -> Option<Seed> {
        let policy = self.policy;
        let host = self
            .by_key
            .values_mut()
            .find(|host| !host.parked.is_empty() && Self::is_free(policy, host, now))?;
        self.parked -= 1;
        host.parked.pop_front()
    }

    /// Records that a request to `key` started.
    pub(crate) fn started(&mut self, key: &str, now: Instant) {
        let host = self.host(key);
        host.in_flight += 1;
        if !host.delay.is_zero() {
            host.next_start = Some(now + host.delay);
        }
    }

    /// Records that a request to `key` finished.
    pub(crate) fn finished(&mut self, key: &str) {
        let host = self.host(key);
        host.in_flight = host.in_flight.saturating_sub(1);
    }

    /// Raises the delay of `key` to at least `delay`, e.g. for a
    /// robots.txt `Crawl-delay`.
    #[cfg_attr(not(feature = "robots"), allow(dead_code, reason = "used by `robots`"))]
    pub(crate) fn raise_delay(&mut self, key: &str, delay: Duration) {
        let host = self.host(key);
        host.delay = host.delay.max(delay);
    }

    /// The earliest instant a parked seed's host becomes free by time alone.
    pub(crate) fn earliest(&self) -> Option<Instant> {
        self.by_key
            .values()
            .filter(|host| !host.parked.is_empty())
            .filter_map(|host| host.next_start)
            .min()
    }

    /// The number of parked seeds.
    pub(crate) const fn parked(&self) -> usize {
        self.parked
    }

    /// Takes every parked seed, e.g. when the run stops.
    pub(crate) fn drain(&mut self) -> Vec<Seed> {
        self.parked = 0;
        self.by_key
            .values_mut()
            .flat_map(|host| host.parked.drain(..))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(uri: &str) -> Seed {
        Seed::get(uri).unwrap()
    }

    fn policy(max: usize, delay: u64) -> HostPolicy {
        HostPolicy {
            max_concurrency: NonZeroUsize::new(max),
            delay: Duration::from_secs(delay),
        }
    }

    #[test]
    fn limits_concurrency_per_host() {
        let now = Instant::now();
        let mut hosts = Hosts::new(policy(1, 0));

        assert!(matches!(
            hosts.admit(seed("https://a.com/1"), now),
            Admit::Start(_)
        ));
        hosts.started("a.com", now);
        assert!(matches!(
            hosts.admit(seed("http://A.com/2"), now),
            Admit::Parked
        ));
        assert!(matches!(
            hosts.admit(seed("https://b.com/"), now),
            Admit::Start(_)
        ));
        assert!(hosts.take_ready(now).is_none());

        hosts.finished("a.com");
        assert_eq!(hosts.take_ready(now).unwrap().uri().path(), "/2");
        assert_eq!(hosts.parked(), 0);
    }

    #[test]
    fn spaces_out_requests_per_host() {
        let now = Instant::now();
        let mut hosts = Hosts::new(policy(0, 2));
        hosts.started("a.com", now);

        assert!(matches!(
            hosts.admit(seed("https://a.com/2"), now),
            Admit::Parked
        ));
        assert_eq!(hosts.earliest(), Some(now + Duration::from_secs(2)));
        assert!(hosts.take_ready(now + Duration::from_secs(1)).is_none());
        assert!(hosts.take_ready(now + Duration::from_secs(2)).is_some());
    }

    #[test]
    fn raised_delay_applies_to_the_next_start() {
        let now = Instant::now();
        let mut hosts = Hosts::new(policy(0, 1));
        hosts.raise_delay("a.com", Duration::from_secs(5));
        hosts.started("a.com", now);

        assert!(matches!(
            hosts.admit(seed("https://a.com/"), now),
            Admit::Parked
        ));
        assert_eq!(hosts.earliest(), Some(now + Duration::from_secs(5)));
    }
}
