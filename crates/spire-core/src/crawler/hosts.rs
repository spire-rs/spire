//! Per-host politeness: concurrency and delays.

use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use tokio::time::Instant;

use crate::crawler::concurrency::{Feedback, HostConcurrency, Limit};
use crate::frontier::seed::Seed;

/// How a crawl treats every host.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct HostPolicy {
    /// Requests in flight per host at most; unlimited if `None`.
    pub(crate) concurrency: Option<HostConcurrency>,
    /// Time between the starts of two requests to the same host.
    pub(crate) delay: Duration,
}

/// What to do with a seed that is allowed to be crawled.
pub(crate) enum Admit {
    /// Start it now.
    Start(Seed),
    /// Its host is busy; it waits in the host's queue.
    Parked,
}

struct Host {
    in_flight: usize,
    next_start: Option<Instant>,
    /// The policy's delay, raised by a robots.txt `Crawl-delay`.
    delay: Duration,
    /// The host's own limit, if limits adapt.
    limit: Limit,
    parked: VecDeque<Seed>,
}

/// The state of every host a run has seen.
///
/// Hosts are keyed by authority, so `http://` and `https://` of the same
/// host share limits: they reach the same server.
pub(crate) struct Hosts {
    policy: HostPolicy,
    /// Whether hosts are tracked at all: without limits or delays, every
    /// host is always free and the bookkeeping is skipped.
    tracked: bool,
    by_key: HashMap<String, Host>,
    parked: usize,
}

/// The key a seed's host is tracked under: its lowercase authority.
fn key(seed: &Seed) -> String {
    seed.uri()
        .authority()
        .map(|authority| authority.as_str().to_ascii_lowercase())
        .unwrap_or_default()
}

impl Hosts {
    /// Tracks hosts if `policy` limits them, or if `delays` may be raised
    /// later, e.g. by a robots.txt `Crawl-delay`.
    pub(crate) fn new(policy: HostPolicy, delays: bool) -> Self {
        let tracked = policy.concurrency.is_some() || !policy.delay.is_zero() || delays;
        Self {
            policy,
            tracked,
            by_key: HashMap::new(),
            parked: 0,
        }
    }

    fn host(&mut self, key: &str) -> &mut Host {
        // Looked up first, so the key is only copied for a new host.
        if !self.by_key.contains_key(key) {
            let host = Host {
                in_flight: 0,
                next_start: None,
                delay: self.policy.delay,
                limit: Limit::new(),
                parked: VecDeque::new(),
            };
            self.by_key.insert(key.to_owned(), host);
        }
        self.by_key
            .get_mut(key)
            .unwrap_or_else(|| unreachable!("the host was just inserted"))
    }

    fn is_free(policy: HostPolicy, host: &Host, now: Instant) -> bool {
        let capacity = policy.concurrency.is_none_or(|concurrency| {
            let limit = if concurrency.is_adaptive() {
                host.limit.get()
            } else {
                concurrency.max()
            };
            host.in_flight < limit
        });
        let due = host.next_start.is_none_or(|at| at <= now);
        capacity && due
    }

    /// Decides whether `seed` starts now or waits for its host.
    pub(crate) fn admit(&mut self, seed: Seed, now: Instant) -> Admit {
        if !self.tracked {
            return Admit::Start(seed);
        }
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

    /// Records that a request for `seed` started, returning the key to pass
    /// to [`finished`](Self::finished), or `None` if hosts are not tracked.
    pub(crate) fn start(&mut self, seed: &Seed, now: Instant) -> Option<String> {
        if !self.tracked {
            return None;
        }
        let key = key(seed);
        self.started(&key, now);
        Some(key)
    }

    /// Records that a request to `key` started.
    pub(crate) fn started(&mut self, key: &str, now: Instant) {
        let host = self.host(key);
        host.in_flight += 1;
        if !host.delay.is_zero() {
            host.next_start = Some(now + host.delay);
        }
    }

    /// Whether limits adapt to responses, which then need [`Feedback`].
    pub(crate) fn adaptive(&self) -> bool {
        self.policy
            .concurrency
            .is_some_and(HostConcurrency::is_adaptive)
    }

    /// Records that a request to `key` finished at `now`, adapting the
    /// host's limit to `feedback` if limits adapt.
    pub(crate) fn finished(&mut self, key: &str, feedback: Option<Feedback>, now: Instant) {
        let concurrency = self.policy.concurrency;
        let host = self.host(key);
        host.in_flight = host.in_flight.saturating_sub(1);
        if let (Some(concurrency), Some(feedback)) = (concurrency, feedback)
            && concurrency.is_adaptive()
        {
            host.limit.update(concurrency.max(), feedback, now);
        }
    }

    /// Raises the delay of `key` to at least `delay`, e.g. for a
    /// robots.txt `Crawl-delay`.
    #[cfg(feature = "robots")]
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
            concurrency: (max > 0).then(|| HostConcurrency::fixed(max)),
            delay: Duration::from_secs(delay),
        }
    }

    #[test]
    fn limits_concurrency_per_host() {
        let now = Instant::now();
        let mut hosts = Hosts::new(policy(1, 0), false);

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

        hosts.finished("a.com", None, now);
        assert_eq!(hosts.take_ready(now).unwrap().uri().path(), "/2");
        assert_eq!(hosts.parked(), 0);
    }

    #[test]
    fn spaces_out_requests_per_host() {
        let now = Instant::now();
        let mut hosts = Hosts::new(policy(0, 2), false);
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
    #[cfg(feature = "robots")]
    fn raised_delay_applies_to_the_next_start() {
        let now = Instant::now();
        let mut hosts = Hosts::new(policy(0, 1), false);
        hosts.raise_delay("a.com", Duration::from_secs(5));
        hosts.started("a.com", now);

        assert!(matches!(
            hosts.admit(seed("https://a.com/"), now),
            Admit::Parked
        ));
        assert_eq!(hosts.earliest(), Some(now + Duration::from_secs(5)));
    }

    #[test]
    fn untracked_hosts_start_everything_at_once() {
        let now = Instant::now();
        let mut hosts = Hosts::new(policy(0, 0), false);
        for _ in 0..3 {
            let seed = seed("https://a.com/");
            assert!(hosts.start(&seed, now).is_none());
            assert!(matches!(hosts.admit(seed, now), Admit::Start(_)));
        }
        assert_eq!((hosts.parked(), hosts.earliest()), (0, None));

        // A delay that may be raised later keeps hosts tracked.
        let mut hosts = Hosts::new(policy(0, 0), true);
        assert_eq!(
            hosts.start(&seed("https://a.com/"), now).as_deref(),
            Some("a.com")
        );
    }

    #[test]
    fn adaptive_limits_gate_each_host() {
        use crate::crawler::concurrency::Signal;

        let now = Instant::now();
        let policy = HostPolicy {
            concurrency: Some(HostConcurrency::adaptive(4)),
            delay: Duration::ZERO,
        };
        let mut hosts = Hosts::new(policy, false);
        assert!(hosts.adaptive());

        // A host starts at one request at a time.
        let first = seed("https://a.com/1");
        assert!(matches!(hosts.admit(first.clone(), now), Admit::Start(_)));
        hosts.start(&first, now);
        assert!(matches!(
            hosts.admit(seed("https://a.com/2"), now),
            Admit::Parked
        ));
        assert!(matches!(
            hosts.admit(seed("https://b.com/"), now),
            Admit::Start(_)
        ));

        // A fast success raises it to two: the parked seed and one more
        // start, and a fourth waits.
        let fast = Feedback {
            started: now,
            latency: Some(Duration::from_millis(100)),
            signal: Signal::Ok,
        };
        hosts.finished("a.com", Some(fast), now);
        assert_eq!(hosts.by_key["a.com"].limit.get(), 2);
        assert_eq!(hosts.take_ready(now).unwrap().uri().path(), "/2");
        hosts.started("a.com", now);
        assert!(matches!(
            hosts.admit(seed("https://a.com/3"), now),
            Admit::Start(_)
        ));
        hosts.started("a.com", now);
        assert!(matches!(
            hosts.admit(seed("https://a.com/4"), now),
            Admit::Parked
        ));
    }
}
